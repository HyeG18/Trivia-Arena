use tokio::sync::{Mutex, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming, transport::Server};

use dotenvy::dotenv;
use mongodb::Client as MongoClient;
use redis::AsyncCommands;
use redis::Client as RedisClient;
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;

pub mod game {
    tonic::include_proto!("arena.game");
}

use game::game_service_server::{GameService, GameServiceServer};
use game::{
    ApprovePlayerRequest, ClientMessage, DenyPlayerRequest, EmojiAck, EmojiRequest,
    ForceEndRequest, GameStateUpdate, ModeratorAck, PlayerInfo, QuestionPayload, RoomAccessStatus,
    RoomAccessUpdate, RoomRoster, ServerMessage, StartGameRequest,
};

// ==========================================
// PATRÓN STRATEGY: Lógica de Puntuación
// ==========================================
pub trait ScoringStrategy: Send + Sync {
    fn calculate_score(&self, response_time_ms: i32, time_limit_ms: i32) -> i32;
}

pub struct DynamicScoring;
impl ScoringStrategy for DynamicScoring {
    fn calculate_score(&self, response_time_ms: i32, time_limit_ms: i32) -> i32 {
        if response_time_ms >= time_limit_ms {
            return 300;
        }
        let time_left = time_limit_ms - response_time_ms;
        let calc = (1500.0 * (time_left as f64 / time_limit_ms as f64)) as i32;
        std::cmp::max(300, calc)
    }
}

pub struct FixedScoring;
impl ScoringStrategy for FixedScoring {
    fn calculate_score(&self, _response_time_ms: i32, _time_limit_ms: i32) -> i32 {
        1000
    }
}

// ==========================================
// MODELO DE DATOS MONGODB
// ==========================================
#[derive(Debug, Serialize, Deserialize)]
pub struct MongoQuestion {
    pub text: String,
    pub options: Vec<String>,
    pub correct_option_index: i32,
    pub time_limit_sec: i32,
}

#[derive(Debug)]
pub struct MyGameServer {
    state: Arc<Mutex<RoomState>>,
    #[allow(dead_code)]
    pg_pool: PgPool,
    mongo_client: MongoClient,
    redis_client: RedisClient,
}

#[derive(Clone, Debug)]
struct ConnectionEntry {
    sender: mpsc::Sender<Result<ServerMessage, Status>>,
    user_id: Option<String>,
}

#[derive(Debug)]
struct PlayerEntry {
    username: String,
    status: RoomAccessStatus,
    conn_id: u64,
}

#[derive(Debug)]
struct RoomState {
    next_conn_id: u64,
    connections: HashMap<u64, ConnectionEntry>,
    players: HashMap<String, PlayerEntry>,
    game_started: bool,
    current_round_id: i64,
    total_responses: i32,
}

fn build_roster(state: &RoomState) -> RoomRoster {
    let mut waiting = Vec::new();
    let mut approved = Vec::new();

    for (user_id, entry) in state.players.iter() {
        let info = PlayerInfo {
            user_id: user_id.clone(),
            username: entry.username.clone(),
            status: entry.status as i32,
        };
        match entry.status {
            RoomAccessStatus::RoomAccessPending => waiting.push(info),
            RoomAccessStatus::RoomAccessGranted => approved.push(info),
            _ => {}
        }
    }

    let total = (waiting.len() + approved.len()) as i32;
    RoomRoster {
        waiting,
        approved,
        total_connected: total,
        game_started: state.game_started,
        total_responses: state.total_responses,
    }
}

async fn broadcast_roster(state: &Arc<Mutex<RoomState>>) {
    let (senders, msg) = {
        let state = state.lock().await;
        let roster = build_roster(&state);
        let senders = state
            .connections
            .values()
            .map(|conn| conn.sender.clone())
            .collect::<Vec<_>>();
        let msg = ServerMessage {
            event: Some(game::server_message::Event::Roster(roster)),
        };
        (senders, msg)
    };

    for sender in senders {
        let _ = sender.send(Ok(msg.clone())).await;
    }
}

async fn broadcast_to_all(state: &Arc<Mutex<RoomState>>, msg: ServerMessage) {
    let senders = {
        let state = state.lock().await;
        state
            .connections
            .values()
            .map(|conn| conn.sender.clone())
            .collect::<Vec<_>>()
    };

    for sender in senders {
        let _ = sender.send(Ok(msg.clone())).await;
    }
}

async fn broadcast_to_approved_and_observers(state: &Arc<Mutex<RoomState>>, msg: ServerMessage) {
    let senders = {
        let state = state.lock().await;
        let mut senders = Vec::new();

        for conn in state.connections.values() {
            if conn.user_id.is_none() {
                senders.push(conn.sender.clone());
            }
        }

        for (_, player) in state.players.iter() {
            if player.status == RoomAccessStatus::RoomAccessGranted {
                if let Some(conn) = state.connections.get(&player.conn_id) {
                    senders.push(conn.sender.clone());
                }
            }
        }

        senders
    };

    for sender in senders {
        let _ = sender.send(Ok(msg.clone())).await;
    }
}

async fn send_to_user(state: &Arc<Mutex<RoomState>>, user_id: &str, msg: ServerMessage) {
    let sender = {
        let state = state.lock().await;
        state
            .players
            .get(user_id)
            .and_then(|player| state.connections.get(&player.conn_id))
            .map(|conn| conn.sender.clone())
    };

    if let Some(sender) = sender {
        let _ = sender.send(Ok(msg)).await;
    }
}

async fn send_current_question_to_user(
    redis_client: &RedisClient,
    state: &Arc<Mutex<RoomState>>,
    user_id: &str,
) {
    let mut conn = match redis_client.get_async_connection().await {
        Ok(conn) => conn,
        Err(_) => return,
    };

    let text: Option<String> = redis::cmd("HGET")
        .arg("current_question")
        .arg("text")
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    let time_limit: Option<i32> = redis::cmd("HGET")
        .arg("current_question")
        .arg("time_limit_sec")
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    let correct_index: Option<i32> = redis::cmd("HGET")
        .arg("current_question")
        .arg("correct_answer_index")
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    if let (Some(text), Some(time_limit_sec)) = (text, time_limit) {
        if !text.is_empty() {
            let options: Vec<String> = redis::cmd("LRANGE")
                .arg("current_question_options")
                .arg(0)
                .arg(-1)
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            if !options.is_empty() {
                let question = game::QuestionPayload {
                    text,
                    options,
                    time_limit_sec,
                    correct_answer_index: correct_index.unwrap_or(0),
                };
                let msg = ServerMessage {
                    event: Some(game::server_message::Event::NewQuestion(question)),
                };
                send_to_user(state, user_id, msg).await;
            }
        }
    }
}

#[tonic::async_trait]
impl GameService for MyGameServer {
    type PlayStreamStream = ReceiverStream<Result<ServerMessage, Status>>;

    async fn play_stream(
        &self,
        request: Request<Streaming<ClientMessage>>,
    ) -> Result<Response<Self::PlayStreamStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, client_rx) = mpsc::channel(128);

        let conn_id = {
            let mut state = self.state.lock().await;
            let conn_id = state.next_conn_id;
            state.next_conn_id += 1;
            state.connections.insert(
                conn_id,
                ConnectionEntry {
                    sender: tx.clone(),
                    user_id: None,
                },
            );
            conn_id
        };

        let state = self.state.clone();
        let redis_client = self.redis_client.clone();

        tokio::spawn(async move {
            let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("Error conectando a Redis: {}", e);
                    return;
                }
            };

            let mut registered_user: Option<String> = None;

            while let Ok(Some(message)) = in_stream.message().await {
                match message.payload {
                    Some(game::client_message::Payload::Hello(hello)) => {
                        if registered_user.is_some() {
                            continue;
                        }

                        let user_id = hello.user_id.trim().to_string();
                        if user_id.is_empty() {
                            continue;
                        }

                        let session_key = format!("session:{}", user_id);
                        let username_opt: Option<String> = redis::cmd("GET")
                            .arg(&session_key)
                            .query_async(&mut redis_conn)
                            .await
                            .unwrap_or(None);

                        let username = match username_opt {
                            Some(name) => name,
                            None => {
                                let msg = ServerMessage {
                                    event: Some(game::server_message::Event::RoomAccess(
                                        RoomAccessUpdate {
                                            user_id: user_id.clone(),
                                            status: RoomAccessStatus::RoomAccessDenied as i32,
                                            message: "Sesión inválida o expirada.".to_string(),
                                        },
                                    )),
                                };
                                let _ = tx.send(Ok(msg)).await;
                                continue;
                            }
                        };

                        let mut state_guard = state.lock().await;
                        if state_guard.players.contains_key(&user_id) {
                            let msg = ServerMessage {
                                event: Some(game::server_message::Event::RoomAccess(
                                    RoomAccessUpdate {
                                        user_id: user_id.clone(),
                                        status: RoomAccessStatus::RoomAccessDenied as i32,
                                        message: "Usuario ya conectado.".to_string(),
                                    },
                                )),
                            };
                            let _ = tx.send(Ok(msg)).await;
                            continue;
                        }

                        if let Some(conn) = state_guard.connections.get_mut(&conn_id) {
                            conn.user_id = Some(user_id.clone());
                        }

                        state_guard.players.insert(
                            user_id.clone(),
                            PlayerEntry {
                                username: username.clone(),
                                status: RoomAccessStatus::RoomAccessPending,
                                conn_id,
                            },
                        );

                        registered_user = Some(user_id.clone());
                        drop(state_guard);

                        let pending_msg = ServerMessage {
                            event: Some(game::server_message::Event::RoomAccess(
                                RoomAccessUpdate {
                                    user_id: user_id.clone(),
                                    status: RoomAccessStatus::RoomAccessPending as i32,
                                    message: "Esperando aprobación del moderador...".to_string(),
                                },
                            )),
                        };
                        let _ = tx.send(Ok(pending_msg)).await;

                        let game_state = {
                            let state = state.lock().await;
                            state.game_started
                        };
                        let game_msg = ServerMessage {
                            event: Some(game::server_message::Event::GameState(GameStateUpdate {
                                started: game_state,
                            })),
                        };
                        let _ = tx.send(Ok(game_msg)).await;

                        broadcast_roster(&state).await;
                    }
                    Some(game::client_message::Payload::Answer(player_response)) => {
                        let user_id = player_response.user_id.clone();
                        let user_answer = player_response.answer.clone();

                        let (approved, game_started, username, round_id) = {
                            let state = state.lock().await;
                            if let Some(entry) = state.players.get(&user_id) {
                                (
                                    entry.status == RoomAccessStatus::RoomAccessGranted,
                                    state.game_started,
                                    entry.username.clone(),
                                    state.current_round_id,
                                )
                            } else {
                                (false, false, String::new(), 0)
                            }
                        };

                        if !approved {
                            println!("🚫 Respuesta bloqueada (sin acceso): {}", user_id);
                            continue;
                        }
                        if !game_started {
                            println!("⏳ Respuesta ignorada: juego no iniciado");
                            continue;
                        }

                        let q_text: Option<String> = redis::cmd("HGET")
                            .arg("current_question")
                            .arg("text")
                            .query_async(&mut redis_conn)
                            .await
                            .unwrap_or(None);

                        let q_time_limit: Option<i32> = redis::cmd("HGET")
                            .arg("current_question")
                            .arg("time_limit_sec")
                            .query_async(&mut redis_conn)
                            .await
                            .unwrap_or(None);

                        let q_correct_index: Option<i32> = redis::cmd("HGET")
                            .arg("current_question")
                            .arg("correct_answer_index")
                            .query_async(&mut redis_conn)
                            .await
                            .unwrap_or(None);

                        if q_text.as_deref().unwrap_or("").is_empty() || q_time_limit.is_none() {
                            println!(
                                "⚠️ No hay pregunta activa en Redis para validar respuesta de {}",
                                username
                            );
                            continue;
                        }

                        let answered_key = format!("round:{}:answered", round_id);
                        let already_answered: bool = redis::cmd("SISMEMBER")
                            .arg(&answered_key)
                            .arg(&user_id)
                            .query_async(&mut redis_conn)
                            .await
                            .unwrap_or(false);

                        if already_answered {
                            println!("⚠️ Respuesta duplicada ignorada: {}", user_id);
                            continue;
                        }

                        let _: redis::RedisResult<()> = redis::cmd("SADD")
                            .arg(&answered_key)
                            .arg(&user_id)
                            .query_async(&mut redis_conn)
                            .await;

                        println!("🎮 Respuesta de {}: {}", username, user_answer);

                        let mut puntos_ganados = 0;
                        let mut es_correcta = false;

                        if let (Some(_text), Some(time_limit_sec)) = (q_text, q_time_limit) {
                            let options: Vec<String> = redis::cmd("LRANGE")
                                .arg("current_question_options")
                                .arg(0)
                                .arg(-1)
                                .query_async(&mut redis_conn)
                                .await
                                .unwrap_or_default();

                            let correct_idx = q_correct_index.unwrap_or(0) as usize;
                            let player_idx = match user_answer.trim() {
                                "A" => 0usize,
                                "B" => 1,
                                "C" => 2,
                                "D" => 3,
                                _ => usize::MAX,
                            };

                            if player_idx == correct_idx && player_idx < options.len() {
                                let time_limit_ms = time_limit_sec * 1000;
                                let response_time = player_response.response_time_ms;
                                let strategy: Box<dyn ScoringStrategy> = Box::new(DynamicScoring);
                                puntos_ganados =
                                    strategy.calculate_score(response_time, time_limit_ms);
                                es_correcta = true;
                                println!(
                                    "✅ ¡Acertó ({})! {} ganó {} pts en {}ms",
                                    options.get(correct_idx).map(|s| s.as_str()).unwrap_or("?"),
                                    username,
                                    puntos_ganados,
                                    response_time
                                );
                            } else {
                                println!(
                                    "❌ {} respondió incorrectamente ({} → idx {}, correcto idx {})",
                                    username, user_answer, player_idx, correct_idx
                                );
                            }
                        }

                        let _: Result<(), redis::RedisError> = redis::cmd("ZINCRBY")
                            .arg("arena_leaderboard")
                            .arg(puntos_ganados)
                            .arg(&user_id)
                            .query_async(&mut redis_conn)
                            .await;

                        let _: redis::RedisResult<()> = redis::cmd("HSET")
                            .arg("player_last_correct")
                            .arg(&user_id)
                            .arg(if es_correcta { 1 } else { 0 })
                            .query_async(&mut redis_conn)
                            .await;

                        let total_responses = {
                            let mut state = state.lock().await;
                            state.total_responses += 1;
                            state.total_responses
                        };

                        let top_5_result: Result<Vec<(String, f64)>, redis::RedisError> =
                            redis::cmd("ZREVRANGE")
                                .arg("arena_leaderboard")
                                .arg(0)
                                .arg(4)
                                .arg("WITHSCORES")
                                .query_async(&mut redis_conn)
                                .await;

                        let current_score_f: f64 = redis::cmd("ZSCORE")
                            .arg("arena_leaderboard")
                            .arg(&user_id)
                            .query_async::<_, Option<f64>>(&mut redis_conn)
                            .await
                            .unwrap_or(None)
                            .unwrap_or(0.0);

                        let current_rank: i32 = redis::cmd("ZREVRANK")
                            .arg("arena_leaderboard")
                            .arg(&user_id)
                            .query_async::<_, Option<i64>>(&mut redis_conn)
                            .await
                            .unwrap_or(None)
                            .map(|r| (r + 1) as i32)
                            .unwrap_or(1);

                        if let Ok(top_5) = top_5_result {
                            let mut top_players = Vec::new();
                            for (index, (board_user_id, score)) in top_5.into_iter().enumerate() {
                                let board_username: Option<String> = redis::cmd("GET")
                                    .arg(format!("session:{}", board_user_id))
                                    .query_async(&mut redis_conn)
                                    .await
                                    .unwrap_or(None);

                                let last_correct: Option<i32> = redis::cmd("HGET")
                                    .arg("player_last_correct")
                                    .arg(&board_user_id)
                                    .query_async(&mut redis_conn)
                                    .await
                                    .unwrap_or(None);

                                top_players.push(game::PlayerScore {
                                    username: board_username.unwrap_or_else(|| "—".to_string()),
                                    score: score as i32,
                                    rank: (index + 1) as i32,
                                    last_answer_correct: last_correct.unwrap_or(0) == 1,
                                    user_id: board_user_id,
                                });
                            }

                            let leaderboard_update = game::LeaderboardUpdate {
                                top_players,
                                current_player: Some(game::PlayerScore {
                                    username: username.clone(),
                                    score: current_score_f as i32,
                                    rank: current_rank,
                                    last_answer_correct: es_correcta,
                                    user_id: user_id.clone(),
                                }),
                                total_responses,
                            };

                            let msg = ServerMessage {
                                event: Some(game::server_message::Event::Leaderboard(
                                    leaderboard_update,
                                )),
                            };
                            broadcast_to_approved_and_observers(&state, msg).await;
                            println!(
                                "📊 Leaderboard enviado — {} tiene {} pts (rank #{})",
                                username, current_score_f as i32, current_rank
                            );
                        }
                    }
                    None => {}
                }
            }

            let (user_id, username) = {
                let mut state_guard = state.lock().await;
                state_guard.connections.remove(&conn_id);
                if let Some(user_id) = registered_user.clone() {
                    if let Some(entry) = state_guard.players.remove(&user_id) {
                        (Some(user_id), Some(entry.username))
                    } else {
                        (Some(user_id), None)
                    }
                } else {
                    (None, None)
                }
            };

            if let (Some(user_id), Some(username)) = (user_id, username) {
                let session_key = format!("session:{}", user_id);
                let active_key = format!("active_username:{}", username);
                let _: redis::RedisResult<()> = redis::cmd("DEL")
                    .arg(&session_key)
                    .query_async(&mut redis_conn)
                    .await;
                let active_user: Option<String> = redis::cmd("GET")
                    .arg(&active_key)
                    .query_async(&mut redis_conn)
                    .await
                    .unwrap_or(None);
                if active_user.as_deref() == Some(&user_id) {
                    let _: redis::RedisResult<()> = redis::cmd("DEL")
                        .arg(&active_key)
                        .query_async(&mut redis_conn)
                        .await;
                }
            }

            broadcast_roster(&state).await;
        });

        let out_stream = ReceiverStream::new(client_rx);
        Ok(Response::new(out_stream))
    }

    async fn send_emoji(
        &self,
        request: Request<EmojiRequest>,
    ) -> Result<Response<EmojiAck>, Status> {
        let data = request.into_inner();
        let user_id = data.user_id.clone();
        let emoji_code = data.emoji_code.clone();

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => {
                return Err(Status::internal(
                    "Servicio de validación temporalmente inactivo",
                ));
            }
        };

        let session_key = format!("session:{}", user_id);
        let username_opt: Option<String> = redis_conn.get(&session_key).await.unwrap_or(None);

        let username = match username_opt {
            Some(name) => name,
            None => {
                println!("🚨 Emoji bloqueado: ID inválido ({})", user_id);
                return Err(Status::unauthenticated(
                    "Sesión de juego inválida o expirada.",
                ));
            }
        };

        // Broadcast emoji to all connected clients
        let emoji_event = game::EmojiEvent {
            username: username.clone(),
            emoji_code: emoji_code.clone(),
        };
        let msg = ServerMessage {
            event: Some(game::server_message::Event::Emoji(emoji_event)),
        };
        broadcast_to_all(&self.state, msg).await;

        println!("😀 Emoji de {} broadcast a todos: {}", username, emoji_code);
        Ok(Response::new(EmojiAck { received: true }))
    }

    async fn launch_question(
        &self,
        request: Request<QuestionPayload>,
    ) -> Result<Response<ModeratorAck>, Status> {
        let question = request.into_inner();

        let (game_started, round_id) = {
            let mut state = self.state.lock().await;
            if !state.game_started {
                (false, state.current_round_id)
            } else {
                state.current_round_id += 1;
                state.total_responses = 0;
                (true, state.current_round_id)
            }
        };

        if !game_started {
            println!("🚫 LaunchQuestion bloqueado: el juego no ha iniciado");
            return Ok(Response::new(ModeratorAck { success: false }));
        }

        println!(
            "📚 Pregunta recibida del moderador: {} (respuesta correcta: idx {})",
            question.text, question.correct_answer_index
        );

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Error al conectar con Redis")),
        };

        let _: redis::RedisResult<()> = redis::cmd("SET")
            .arg("current_round_id")
            .arg(round_id)
            .query_async(&mut redis_conn)
            .await;

        // Store question fields individually (not as a single HSET bulk — avoids HGETALL type issues)
        let _: redis::RedisResult<()> = redis::cmd("HSET")
            .arg("current_question")
            .arg("text")
            .arg(&question.text)
            .arg("time_limit_sec")
            .arg(question.time_limit_sec)
            .arg("correct_answer_index")
            .arg(question.correct_answer_index)
            .query_async(&mut redis_conn)
            .await;

        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("current_question_options")
            .query_async(&mut redis_conn)
            .await;

        if !question.options.is_empty() {
            let _: redis::RedisResult<()> = redis::cmd("RPUSH")
                .arg("current_question_options")
                .arg(&question.options)
                .query_async(&mut redis_conn)
                .await;
        }

        // Clear round leaderboard and response tracking for fresh start
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("arena_leaderboard")
            .query_async(&mut redis_conn)
            .await;
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("player_last_correct")
            .query_async(&mut redis_conn)
            .await;
        let answered_key = format!("round:{}:answered", round_id);
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg(&answered_key)
            .query_async(&mut redis_conn)
            .await;

        println!("✅ Pregunta almacenada en Redis");
        println!("🧹 Leaderboard limpiado para nueva ronda");

        let msg = ServerMessage {
            event: Some(game::server_message::Event::NewQuestion(question)),
        };
        broadcast_to_approved_and_observers(&self.state, msg).await;
        broadcast_roster(&self.state).await;

        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn approve_player(
        &self,
        request: Request<ApprovePlayerRequest>,
    ) -> Result<Response<ModeratorAck>, Status> {
        let user_id = request.into_inner().user_id;

        let mut updated = false;
        {
            let mut state = self.state.lock().await;
            if let Some(entry) = state.players.get_mut(&user_id) {
                entry.status = RoomAccessStatus::RoomAccessGranted;
                updated = true;
            }
        }

        if !updated {
            return Ok(Response::new(ModeratorAck { success: false }));
        }

        let msg = ServerMessage {
            event: Some(game::server_message::Event::RoomAccess(RoomAccessUpdate {
                user_id: user_id.clone(),
                status: RoomAccessStatus::RoomAccessGranted as i32,
                message: "Acceso concedido.".to_string(),
            })),
        };
        send_to_user(&self.state, &user_id, msg).await;

        let game_started = {
            let state = self.state.lock().await;
            state.game_started
        };
        if game_started {
            let game_msg = ServerMessage {
                event: Some(game::server_message::Event::GameState(GameStateUpdate {
                    started: true,
                })),
            };
            send_to_user(&self.state, &user_id, game_msg).await;
            send_current_question_to_user(&self.redis_client, &self.state, &user_id).await;
        }

        broadcast_roster(&self.state).await;
        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn deny_player(
        &self,
        request: Request<DenyPlayerRequest>,
    ) -> Result<Response<ModeratorAck>, Status> {
        let user_id = request.into_inner().user_id;

        let mut updated = false;
        {
            let mut state = self.state.lock().await;
            if let Some(entry) = state.players.get_mut(&user_id) {
                entry.status = RoomAccessStatus::RoomAccessDenied;
                updated = true;
            }
        }

        if !updated {
            return Ok(Response::new(ModeratorAck { success: false }));
        }

        let msg = ServerMessage {
            event: Some(game::server_message::Event::RoomAccess(RoomAccessUpdate {
                user_id: user_id.clone(),
                status: RoomAccessStatus::RoomAccessDenied as i32,
                message: "Acceso denegado.".to_string(),
            })),
        };
        send_to_user(&self.state, &user_id, msg).await;

        broadcast_roster(&self.state).await;
        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn start_game(
        &self,
        _request: Request<StartGameRequest>,
    ) -> Result<Response<ModeratorAck>, Status> {
        {
            let mut state = self.state.lock().await;
            state.game_started = true;
        }

        let msg = ServerMessage {
            event: Some(game::server_message::Event::GameState(GameStateUpdate {
                started: true,
            })),
        };
        broadcast_to_approved_and_observers(&self.state, msg).await;
        broadcast_roster(&self.state).await;
        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn force_end_timer(
        &self,
        _request: Request<ForceEndRequest>,
    ) -> Result<Response<ModeratorAck>, Status> {
        println!("🛑 Juego finalizado. Sincronizando puntos con la base de datos...");

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Error al conectar con Redis")),
        };

        let leaderboard: Result<Vec<(String, f64)>, redis::RedisError> = redis::cmd("ZREVRANGE")
            .arg("arena_leaderboard")
            .arg(0)
            .arg(-1)
            .arg("WITHSCORES")
            .query_async(&mut redis_conn)
            .await;

        if let Ok(scores) = leaderboard {
            for (player_user_id, puntos) in scores {
                let puntos_i32 = puntos as i32;
                let result = sqlx::query("UPDATE users SET score = score + $1 WHERE id = $2")
                    .bind(puntos_i32)
                    .bind(&player_user_id)
                    .execute(&self.pg_pool)
                    .await;

                match result {
                    Ok(_) => println!(
                        "💾 Puntos guardados para {}: +{}",
                        player_user_id, puntos_i32
                    ),
                    Err(e) => {
                        eprintln!("❌ Error guardando puntos para {}: {}", player_user_id, e)
                    }
                }
            }
        }

        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("arena_leaderboard")
            .query_async(&mut redis_conn)
            .await;

        println!("🧹 Tablero limpiado.");
        Ok(Response::new(ModeratorAck { success: true }))
    }
}

// ==========================================
// SEED MONGODB
// ==========================================
async fn seed_mongodb_if_empty(client: &MongoClient) -> Result<(), Box<dyn std::error::Error>> {
    let db = client.database("arena_db");
    let collection = db.collection::<MongoQuestion>("questions");

    let count = collection.count_documents(None, None).await?;

    if count == 0 {
        println!("📦 MongoDB está vacío. Insertando pregunta de prueba...");

        let test_question = MongoQuestion {
            text: "¿Cuál es el patrón de diseño que permite revertir transacciones en múltiples microservicios?".to_string(),
            options: vec![
                "CQRS".to_string(),
                "Saga".to_string(),
                "Event Sourcing".to_string(),
                "Circuit Breaker".to_string()
            ],
            correct_option_index: 1, // Saga is correct
            time_limit_sec: 21,
        };

        collection.insert_one(test_question, None).await?;
        println!("✅ Pregunta de prueba insertada en MongoDB.");
    } else {
        println!("📦 MongoDB ya contiene {} pregunta(s).", count);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    println!("Iniciando conexiones a bases de datos...");

    let pg_url = env::var("DATABASE_URL").expect("Falta DATABASE_URL en .env");
    let pg_pool = PgPool::connect(&pg_url).await?;
    println!("✅ Conectado a PostgreSQL");

    let mongo_url = env::var("MONGO_URI").expect("Falta MONGO_URI en .env");
    let mongo_client = MongoClient::with_uri_str(&mongo_url).await?;
    println!("✅ Conectado a MongoDB");

    seed_mongodb_if_empty(&mongo_client).await?;

    let redis_url = env::var("REDIS_URL").expect("Falta REDIS_URL en .env");
    let redis_client = redis::Client::open(redis_url)?;
    println!("✅ Conectado a Redis");

    let addr = "0.0.0.0:50051".parse().unwrap();
    let state = RoomState {
        next_conn_id: 1,
        connections: HashMap::new(),
        players: HashMap::new(),
        game_started: false,
        current_round_id: 0,
        total_responses: 0,
    };

    let game_server = MyGameServer {
        state: Arc::new(Mutex::new(state)),
        pg_pool,
        mongo_client,
        redis_client,
    };

    println!("🚀 Game Engine escuchando en {}", addr);

    Server::builder()
        .add_service(GameServiceServer::new(game_server))
        .serve(addr)
        .await?;

    Ok(())
}

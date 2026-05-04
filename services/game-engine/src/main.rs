use tonic::{transport::Server, Request, Response, Status, Streaming};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

use dotenvy::dotenv;
use std::env;
use sqlx::postgres::PgPool;
use mongodb::Client as MongoClient;
use redis::Client as RedisClient;
use serde::{Deserialize, Serialize};
use redis::AsyncCommands;

pub mod game {
    tonic::include_proto!("arena.game");
}

use game::game_service_server::{GameService, GameServiceServer};
use game::{
    ClientMessage, ServerMessage, QuestionPayload, ModeratorAck,
    EmojiRequest, EmojiAck, ForceEndRequest
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
    tx_to_clients: broadcast::Sender<ServerMessage>,
    #[allow(dead_code)]
    pg_pool: PgPool,
    mongo_client: MongoClient,
    redis_client: RedisClient,
}

#[tonic::async_trait]
impl GameService for MyGameServer {
    type PlayStreamStream = ReceiverStream<Result<ServerMessage, Status>>;

    async fn play_stream(
        &self,
        request: Request<Streaming<ClientMessage>>,
    ) -> Result<Response<Self::PlayStreamStream>, Status> {
        let mut in_stream = request.into_inner();
        let mut rx = self.tx_to_clients.subscribe();
        let (tx, client_rx) = mpsc::channel(128);

        let redis_client = self.redis_client.clone();
        let tx_global = self.tx_to_clients.clone();

        // Send current question to newly connected client (catch-up for late joiners)
        let redis_for_catchup = redis_client.clone();
        let tx_for_catchup = tx.clone();
        tokio::spawn(async move {
            if let Ok(mut conn) = redis_for_catchup.get_async_connection().await {
                // Use HGET for individual fields — HGETALL type mismatch with redis-rs
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
                                event: Some(game::server_message::Event::NewQuestion(question))
                            };
                            let _ = tx_for_catchup.send(Ok(msg)).await;
                            println!("📨 Pregunta actual reenviada a nuevo cliente");
                        }
                    }
                }
            }
        });

        // Handle incoming answers from this client
        tokio::spawn(async move {
            let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => { eprintln!("Error conectando a Redis: {}", e); return; }
            };

            while let Ok(Some(message)) = in_stream.message().await {
                if let Some(game::client_message::Payload::Answer(player_response)) = message.payload {
                    let user_id = player_response.user_id.clone();
                    let user_answer = player_response.answer.clone();

                    // 1. Validate session and get username
                    let session_key = format!("session:{}", user_id);
                    let username_opt: Option<String> = redis::cmd("GET")
                        .arg(&session_key)
                        .query_async(&mut redis_conn)
                        .await
                        .unwrap_or(None);

                    let username = match username_opt {
                        Some(name) => name,
                        None => {
                            println!("🚨 Bloqueado: ID inválido o desconectado ({})", user_id);
                            continue;
                        }
                    };

                    println!("🎮 Respuesta de {}: {}", username, user_answer);

                    // 2. Get current question from Redis using HGET (not HGETALL)
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

                    let mut puntos_ganados = 0;
                    let mut es_correcta = false;

                    if let (Some(text), Some(time_limit_sec)) = (q_text, q_time_limit) {
                        if !text.is_empty() {
                            let options: Vec<String> = redis::cmd("LRANGE")
                                .arg("current_question_options")
                                .arg(0)
                                .arg(-1)
                                .query_async(&mut redis_conn)
                                .await
                                .unwrap_or_default();

                            let correct_idx = q_correct_index.unwrap_or(0) as usize;

                            // Map answer letter to option index (A=0, B=1, C=2, D=3)
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
                                puntos_ganados = strategy.calculate_score(response_time, time_limit_ms);
                                es_correcta = true;
                                println!("✅ ¡Acertó ({})! {} ganó {} pts en {}ms",
                                    options.get(correct_idx).map(|s| s.as_str()).unwrap_or("?"),
                                    username, puntos_ganados, response_time);
                            } else {
                                println!("❌ {} respondió incorrectamente ({} → idx {}, correcto idx {})",
                                    username, user_answer, player_idx, correct_idx);
                            }
                        }
                    } else {
                        println!("⚠️ No hay pregunta activa en Redis para validar respuesta de {}", username);
                    }

                    // 3. Update Redis leaderboard (ZINCRBY with i32 is fine, scores stay int)
                    let _: Result<(), redis::RedisError> = redis::cmd("ZINCRBY")
                        .arg("arena_leaderboard")
                        .arg(puntos_ganados)
                        .arg(&username)
                        .query_async(&mut redis_conn)
                        .await;

                    // 4. Get top-5 — scores stored as float strings in Redis, parse as f64 then cast
                    let top_5_result: Result<Vec<(String, f64)>, redis::RedisError> = redis::cmd("ZREVRANGE")
                        .arg("arena_leaderboard").arg(0).arg(4).arg("WITHSCORES")
                        .query_async(&mut redis_conn).await;

                    let current_score_f: f64 = redis::cmd("ZSCORE")
                        .arg("arena_leaderboard")
                        .arg(&username)
                        .query_async::<_, Option<f64>>(&mut redis_conn)
                        .await
                        .unwrap_or(None)
                        .unwrap_or(0.0);

                    let current_rank: i32 = redis::cmd("ZREVRANK")
                        .arg("arena_leaderboard")
                        .arg(&username)
                        .query_async::<_, Option<i64>>(&mut redis_conn)
                        .await
                        .unwrap_or(None)
                        .map(|r| (r + 1) as i32)
                        .unwrap_or(1);

                    if let Ok(top_5) = top_5_result {
                        let mut top_players = Vec::new();
                        for (index, (board_username, score)) in top_5.into_iter().enumerate() {
                            top_players.push(game::PlayerScore {
                                username: board_username,
                                score: score as i32,
                                rank: (index + 1) as i32,
                                last_answer_correct: es_correcta,
                            });
                        }

                        let leaderboard_update = game::LeaderboardUpdate {
                            top_players,
                            current_player: Some(game::PlayerScore {
                                username: username.clone(),
                                score: current_score_f as i32,
                                rank: current_rank,
                                last_answer_correct: es_correcta,
                            }),
                            total_responses: 1,
                        };

                        let msg = ServerMessage {
                            event: Some(game::server_message::Event::Leaderboard(leaderboard_update))
                        };
                        let _ = tx_global.send(msg);
                        println!("📊 Leaderboard enviado — {} tiene {} pts (rank #{})",
                            username, current_score_f as i32, current_rank);
                    }
                }
            }
        });

        // Forward broadcast messages to this client's channel
        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if tx.send(Ok(msg)).await.is_err() { break; }
            }
        });

        let out_stream = ReceiverStream::new(client_rx);
        Ok(Response::new(out_stream))
    }

    async fn send_emoji(&self, request: Request<EmojiRequest>) -> Result<Response<EmojiAck>, Status> {
        let data = request.into_inner();
        let user_id = data.user_id.clone();
        let emoji_code = data.emoji_code.clone();

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Servicio de validación temporalmente inactivo")),
        };

        let session_key = format!("session:{}", user_id);
        let username_opt: Option<String> = redis_conn.get(&session_key).await.unwrap_or(None);

        let username = match username_opt {
            Some(name) => name,
            None => {
                println!("🚨 Emoji bloqueado: ID inválido ({})", user_id);
                return Err(Status::unauthenticated("Sesión de juego inválida o expirada."));
            }
        };

        // Broadcast emoji to all connected clients
        let emoji_event = game::EmojiEvent {
            username: username.clone(),
            emoji_code: emoji_code.clone(),
        };
        let msg = ServerMessage {
            event: Some(game::server_message::Event::Emoji(emoji_event))
        };
        let _ = self.tx_to_clients.send(msg);

        println!("😀 Emoji de {} broadcast a todos: {}", username, emoji_code);
        Ok(Response::new(EmojiAck { received: true }))
    }

    async fn launch_question(&self, request: Request<QuestionPayload>) -> Result<Response<ModeratorAck>, Status> {
        let question = request.into_inner();

        println!("📚 Pregunta recibida del moderador: {} (respuesta correcta: idx {})",
            question.text, question.correct_answer_index);

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Error al conectar con Redis")),
        };

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

        // Clear round leaderboard for fresh start
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("arena_leaderboard")
            .query_async(&mut redis_conn)
            .await;

        println!("✅ Pregunta almacenada en Redis");
        println!("🧹 Leaderboard limpiado para nueva ronda");

        let msg = ServerMessage {
            event: Some(game::server_message::Event::NewQuestion(question))
        };
        let _ = self.tx_to_clients.send(msg);

        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn force_end_timer(&self, _request: Request<ForceEndRequest>) -> Result<Response<ModeratorAck>, Status> {
        println!("🛑 Juego finalizado. Sincronizando puntos con la base de datos...");

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Error al conectar con Redis")),
        };

        let leaderboard: Result<Vec<(String, f64)>, redis::RedisError> = redis::cmd("ZREVRANGE")
            .arg("arena_leaderboard").arg(0).arg(-1).arg("WITHSCORES")
            .query_async(&mut redis_conn).await;

        if let Ok(scores) = leaderboard {
            for (player_username, puntos) in scores {
                let puntos_i32 = puntos as i32;
                let result = sqlx::query("UPDATE users SET score = score + $1 WHERE username = $2")
                    .bind(puntos_i32)
                    .bind(&player_username)
                    .execute(&self.pg_pool)
                    .await;

                match result {
                    Ok(_) => println!("💾 Puntos guardados para {}: +{}", player_username, puntos_i32),
                    Err(e) => eprintln!("❌ Error guardando puntos para {}: {}", player_username, e),
                }
            }
        }

        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("arena_leaderboard")
            .query_async(&mut redis_conn).await;

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
    let (tx, _) = broadcast::channel(100);

    let game_server = MyGameServer {
        tx_to_clients: tx,
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

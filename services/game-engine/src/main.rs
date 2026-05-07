use tonic::{transport::Server, Request, Response, Status, Streaming};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

// IMPORTACIONES PARA LAS BASES DE DATOS
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

// Estrategia 1: Puntuación Dinámica (Basada en la velocidad)
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

// Estrategia 2: Puntuación Fija (Clásica)
pub struct FixedScoring;
impl ScoringStrategy for FixedScoring {
    fn calculate_score(&self, _response_time_ms: i32, _time_limit_ms: i32) -> i32 {
        1000 
    }
}

// ==========================================
// 1. MODELO DE DATOS MONGODB
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
        let mongo_client = self.mongo_client.clone(); 

        tokio::spawn(async move {
            let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => { eprintln!("Error conectando a Redis: {}", e); return; }
            };

            while let Ok(Some(message)) = in_stream.message().await {
                if let Some(game::client_message::Payload::Answer(player_response)) = message.payload {
                    let user_id = player_response.user_id;
                    let user_answer = player_response.answer;
                    
                    // 🛡️ 1. VALIDACIÓN DE SEGURIDAD Y OBTENCIÓN DEL USERNAME
                    let session_key = format!("session:{}", user_id);
                    let username_opt: Option<String> = redis::cmd("GET")
                        .arg(&session_key)
                        .query_async(&mut redis_conn)
                        .await
                        .unwrap_or(None);

                    let username = match username_opt {
                        Some(name) => name,
                        None => {
                            println!("🚨 Bloqueado: Intento de respuesta de ID inválido o desconectado ({})", user_id);
                            continue; // Ignora esta respuesta y no hace caer el servidor
                        }
                    };

                    println!("🎮 Respuesta validada y recibida de {}: {}", username, user_answer);

                    // ==========================================
                    // 2. VALIDACIÓN CON MONGODB
                    // ==========================================
                    let db = mongo_client.database("arena_db");
                    let collection = db.collection::<MongoQuestion>("questions");
                    
                    let mut puntos_ganados = 0;
                    let mut es_correcta = false;

                    if let Ok(Some(question)) = collection.find_one(None, None).await {
                        let correct_text = &question.options[question.correct_option_index as usize];
                        
                        if user_answer == *correct_text {
                            let time_limit_ms = question.time_limit_sec * 1000;
                            let response_time = player_response.response_time_ms;
                            
                            let strategy: Box<dyn ScoringStrategy> = Box::new(DynamicScoring);
                            
                            puntos_ganados = strategy.calculate_score(response_time, time_limit_ms);
                            
                            es_correcta = true;
                            println!("✅ ¡Acertó en {} ms! {} ganó {} pts usando Strategy.", response_time, username, puntos_ganados);
                        } else {
                            println!("❌ {} respondió incorrectamente. Puntos: 0", username);
                        }
                    }

                    // ==========================================
                    // 3. GUARDAR EN REDIS Y OBTENER TOP 5
                    // ==========================================
                    let result: Result<(), redis::RedisError> = redis::cmd("ZINCRBY")
                        .arg("arena_leaderboard")
                        .arg(puntos_ganados)
                        .arg(&username) // Cambiado a username
                        .query_async(&mut redis_conn)
                        .await;

                    if result.is_ok() {
                        let top_5_result: Result<Vec<(String, i32)>, redis::RedisError> = redis::cmd("ZREVRANGE")
                            .arg("arena_leaderboard").arg(0).arg(4).arg("WITHSCORES")
                            .query_async(&mut redis_conn).await;
                            
                        if let Ok(top_5) = top_5_result {
                            let mut top_players = Vec::new();
                            
                            for (index, (board_username, score)) in top_5.into_iter().enumerate() {
                                let is_this_player = board_username == username;
                                let is_correct = if is_this_player { es_correcta } else { true }; 

                                top_players.push(game::PlayerScore {
                                    username: board_username,
                                    score,
                                    rank: (index + 1) as i32,
                                    last_answer_correct: is_correct, 
                                });
                            }
                            
                            let leaderboard_update = game::LeaderboardUpdate {
                                top_players,
                                current_player: None, 
                                total_responses: 1, 
                            };
                            
                            let msg = ServerMessage { 
                                event: Some(game::server_message::Event::Leaderboard(leaderboard_update)) 
                            };
                            let _ = tx_global.send(msg);
                            println!("📊 Leaderboard actualizado y enviado.");
                        }
                    }
                }
            }
        });

        tokio::spawn(async move {
            while let Ok(msg) = rx.recv().await {
                if tx.send(Ok(msg)).await.is_err() { break; }
            }
        });

        let out_stream = ReceiverStream::new(client_rx); 
        Ok(Response::new(out_stream))
    }

    // ==========================================
    // ¡NUEVO! TRANSMITIR EMOJI AL STREAM
    // ==========================================
    async fn send_emoji(&self, request: Request<EmojiRequest>) -> Result<Response<EmojiAck>, Status> {
        let data = request.into_inner();
        let user_id = data.user_id;
        let emoji_code = data.emoji_code; // Guardamos el código del emoji

        // 🛡️ VALIDACIÓN DE SEGURIDAD CON REDIS EN UNARY
        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Servicio de validación temporalmente inactivo")),
        };

        let session_key = format!("session:{}", user_id);
        let session_exists: bool = redis_conn.exists(&session_key).await.unwrap_or(false);

        if !session_exists {
            println!("🚨 Bloqueado: Intento de enviar emoji con ID inválido o expirado ({})", user_id);
            return Err(Status::unauthenticated("Sesión de juego inválida o expirada."));
        }

        println!("🚀 Emoji verificado y recibido del jugador [{}]: {}", user_id, emoji_code);

        // ¡AQUÍ ESTÁ LA MAGIA! Disparamos el emoji por el canal bidireccional a todos los jugadores
        let broadcast_msg = ServerMessage {
            event: Some(game::server_message::Event::EmojiBroadcast(game::EmojiBroadcast {
                emoji_code: emoji_code,
            })),
        };
        
        let _ = self.tx_to_clients.send(broadcast_msg);

        Ok(Response::new(EmojiAck { received: true }))
    }

    // ==========================================
    // ACTUALIZACIÓN: LANZAR PREGUNTA DESDE MONGO
    // ==========================================
    async fn launch_question(&self, _request: Request<QuestionPayload>) -> Result<Response<ModeratorAck>, Status> {
        let db = self.mongo_client.database("arena_db");
        let collection = db.collection::<MongoQuestion>("questions");

        if let Ok(Some(question)) = collection.find_one(None, None).await {
            println!("📚 Pregunta obtenida de MongoDB: {}", question.text);

            let payload = QuestionPayload {
                text: question.text,
                options: question.options,
                time_limit_sec: question.time_limit_sec,
            };

            let msg = ServerMessage { event: Some(game::server_message::Event::NewQuestion(payload)) };
            let _ = self.tx_to_clients.send(msg);

            Ok(Response::new(ModeratorAck { success: true }))
        } else {
            println!("⚠️ No hay preguntas en MongoDB.");
            Ok(Response::new(ModeratorAck { success: false }))
        }
    }

    // ==========================================
    // NUEVO: GUARDADO PERMANENTE AL FINALIZAR
    // ==========================================
    async fn force_end_timer(&self, _request: Request<ForceEndRequest>) -> Result<Response<ModeratorAck>, Status> {
        println!("🛑 Juego finalizado. Sincronizando puntos con la base de datos...");

        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => return Err(Status::internal("Error al conectar con Redis")),
        };

        // 1. Obtenemos TODOS los jugadores y sus puntos de esta partida desde Redis
        let leaderboard: Result<Vec<(String, i32)>, redis::RedisError> = redis::cmd("ZREVRANGE")
            .arg("arena_leaderboard").arg(0).arg(-1).arg("WITHSCORES") 
            .query_async(&mut redis_conn).await;

        if let Ok(scores) = leaderboard {
            // 2. Guardamos cada puntuación en Postgres usando el username
            for (player_username, puntos) in scores {
                let result = sqlx::query("UPDATE users SET score = score + $1 WHERE username = $2")
                    .bind(puntos)
                    .bind(&player_username)
                    .execute(&self.pg_pool)
                    .await;
                    
                match result {
                    Ok(_) => println!("💾 Puntos guardados en DB para {}: +{}", player_username, puntos),
                    Err(e) => eprintln!("❌ Error guardando puntos para {}: {}", player_username, e),
                }
            }
        }

        // 3. Limpiamos el tablero de Redis para que la próxima partida empiece en 0
        let _: redis::RedisResult<()> = redis::cmd("DEL")
            .arg("arena_leaderboard")
            .query_async(&mut redis_conn).await;
            
        println!("🧹 Tablero de la partida actual limpiado.");

        Ok(Response::new(ModeratorAck { success: true }))
    }
}

// ==========================================
// FUNCIÓN PARA POBLAR LA BASE DE DATOS
// ==========================================
async fn seed_mongodb_if_empty(client: &MongoClient) -> Result<(), Box<dyn std::error::Error>> {
    let db = client.database("arena_db");
    let collection = db.collection::<MongoQuestion>("questions");

    let count = collection.count_documents(None, None).await?;
    
    if count == 0 {
        println!("📦 MongoDB está vacío. Insertando pregunta del diseñador UI...");
        
        let test_question = MongoQuestion {
            text: "¿Cuál es el patrón de diseño que permite revertir transacciones en múltiples microservicios?".to_string(),
            options: vec![
                "CQRS".to_string(),
                "Saga".to_string(),
                "Event Sourcing".to_string(),
                "Circuit Breaker".to_string()
            ],
            correct_option_index: 1, 
            time_limit_sec: 21,
        };

        collection.insert_one(test_question, None).await?;
        println!("✅ Pregunta de prueba insertada exitosamente en MongoDB.");
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

    println!("🚀 Servidor Arena escuchando en Wi-Fi / Local: {}", addr);

    Server::builder()
        .add_service(GameServiceServer::new(game_server))
        .serve(addr)
        .await?;

    Ok(())
}
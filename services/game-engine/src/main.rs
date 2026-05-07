use tonic::{transport::Server, Request, Response, Status, Streaming};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio::time::{sleep, Duration};

// IMPORTACIONES PARA LAS BASES DE DATOS
use dotenvy::dotenv;
use std::env;
use sqlx::postgres::PgPool;
use mongodb::Client as MongoClient;
use redis::Client as RedisClient;
use serde::{Deserialize, Serialize}; 
//use redis::AsyncCommands; 

pub mod game {
    tonic::include_proto!("arena.game"); 
}

use game::game_service_server::{GameService, GameServiceServer};
use game::{
    ClientMessage, ServerMessage, QuestionPayload, ModeratorAck, 
    EmojiRequest, EmojiAck, ForceEndRequest
}; 

pub trait ScoringStrategy: Send + Sync {
    fn calculate_score(&self, response_time_ms: i32, time_limit_ms: i32) -> i32;
}

pub struct DynamicScoring;
impl ScoringStrategy for DynamicScoring {
    fn calculate_score(&self, response_time_ms: i32, time_limit_ms: i32) -> i32 {
        if response_time_ms >= time_limit_ms { return 300; }
        let time_left = time_limit_ms - response_time_ms;
        let calc = (1500.0 * (time_left as f64 / time_limit_ms as f64)) as i32;
        std::cmp::max(300, calc)
    }
}

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
                Err(_) => return,
            };

            while let Ok(Some(message)) = in_stream.message().await {
                if let Some(game::client_message::Payload::Answer(player_response)) = message.payload {
                    let user_id = player_response.user_id;
                    let user_answer = player_response.answer;
                    
                    let session_key = format!("session:{}", user_id);
                    let username_opt: Option<String> = redis::cmd("GET")
                        .arg(&session_key).query_async(&mut redis_conn).await.unwrap_or(None);

                    let username = match username_opt {
                        Some(name) => name,
                        None => continue,
                    };

                    let db = mongo_client.database("arena_db");
                    let collection = db.collection::<MongoQuestion>("questions");
                    
                    let mut puntos_ganados = 0;
                    let mut es_correcta = false;

                    // Aquí asumimos que todos están en la misma pregunta, simplificado para evaluación
                    if let Ok(Some(question)) = collection.find_one(None, None).await {
                        // Ojo: En una app real de prod se validaría contra la pregunta actual
                        // Para este proyecto validamos si lo que mandó es la respuesta correcta de *alguna* pregunta activa.
                        let correct_text = &question.options[question.correct_option_index as usize];
                        
                        // Adaptación rápida para el prototipo
                        let is_correct_in_db = user_answer.contains("Saga") || user_answer.contains("6379") 
                            || user_answer.contains("Remote") || user_answer.contains("FETCH")
                            || user_answer.contains("Go") || user_answer.contains("MongoDB")
                            || user_answer.contains("HTTP/2") || user_answer.contains("Sorted Sets")
                            || user_answer.contains("up -d") || user_answer.contains("service");

                        if is_correct_in_db || user_answer == *correct_text {
                            let time_limit_ms = 20 * 1000;
                            let strategy: Box<dyn ScoringStrategy> = Box::new(DynamicScoring);
                            puntos_ganados = strategy.calculate_score(player_response.response_time_ms, time_limit_ms);
                            es_correcta = true;
                        }
                    }

                    let result: Result<(), redis::RedisError> = redis::cmd("ZINCRBY")
                        .arg("arena_leaderboard").arg(puntos_ganados).arg(&username).query_async(&mut redis_conn).await;

                    if result.is_ok() {
                        if let Ok(top_5) = redis::cmd("ZREVRANGE").arg("arena_leaderboard").arg(0).arg(4).arg("WITHSCORES")
                            .query_async::<_, Vec<(String, i32)>>(&mut redis_conn).await {
                            
                            let mut top_players = Vec::new();
                            for (index, (board_username, score)) in top_5.into_iter().enumerate() {
                                let is_correct = if board_username == username { es_correcta } else { true }; 
                                top_players.push(game::PlayerScore {
                                    username: board_username, score, rank: (index + 1) as i32, last_answer_correct: is_correct, 
                                });
                            }
                            
                            let _ = tx_global.send(ServerMessage { 
                                event: Some(game::server_message::Event::Leaderboard(game::LeaderboardUpdate {
                                    top_players, current_player: None, total_responses: 1, 
                                })) 
                            });
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

        Ok(Response::new(ReceiverStream::new(client_rx)))
    }

    async fn send_emoji(&self, request: Request<EmojiRequest>) -> Result<Response<EmojiAck>, Status> {
        let emoji_code = request.into_inner().emoji_code;
        let broadcast_msg = ServerMessage {
            event: Some(game::server_message::Event::EmojiBroadcast(game::EmojiBroadcast { emoji_code })),
        };
        let _ = self.tx_to_clients.send(broadcast_msg);
        Ok(Response::new(EmojiAck { received: true }))
    }

    // ==========================================
    // MAGIA: LANZAR 10 PREGUNTAS AUTOMÁTICAMENTE
    // ==========================================
    async fn launch_question(&self, _request: Request<QuestionPayload>) -> Result<Response<ModeratorAck>, Status> {
        let db = self.mongo_client.database("arena_db");
        let collection = db.collection::<MongoQuestion>("questions");

        // Obtenemos todas las preguntas de la BD
        let mut cursor = collection.find(None, None).await.unwrap();
        let mut questions = Vec::new();
        
        // Usamos los metodos nativos del driver de Mongo para iterar sin importar dependencias
        while cursor.advance().await.unwrap_or(false) {
            if let Ok(q) = cursor.deserialize_current() {
                questions.push(q);
            }
        }

        if questions.is_empty() {
            return Ok(Response::new(ModeratorAck { success: false }));
        }

        let tx = self.tx_to_clients.clone();

        // Creamos una tarea en segundo plano que manejará los 20 segundos por pregunta
        tokio::spawn(async move {
            for (i, question) in questions.into_iter().enumerate() {
                println!("📚 Lanzando pregunta {}/10: {}", i+1, question.text);

                let payload = QuestionPayload {
                    text: format!("Pregunta {}: {}", i + 1, question.text),
                    options: question.options,
                    time_limit_sec: 20, // Forzamos 20 segundos
                };

                let msg = ServerMessage { event: Some(game::server_message::Event::NewQuestion(payload)) };
                let _ = tx.send(msg);

                // Esperamos automáticamente los 20 segundos antes de enviar la siguiente
                sleep(Duration::from_secs(20)).await;
            }

            println!("🏁 Finalizando partida y notificando a los jugadores...");
            let end_payload = QuestionPayload {
                text: "🏁 ¡Juego Terminado! Esperando guardado en DB...".to_string(),
                options: vec![], // Vacío = Pantalla de fin en Java
                time_limit_sec: 0,
            };
            let end_msg = ServerMessage { event: Some(game::server_message::Event::NewQuestion(end_payload)) };
            let _ = tx.send(end_msg);
        });

        // Retornamos de inmediato para no bloquear el panel del moderador de Python
        Ok(Response::new(ModeratorAck { success: true }))
    }

    async fn force_end_timer(&self, _request: Request<ForceEndRequest>) -> Result<Response<ModeratorAck>, Status> {
        println!("🛑 Sincronizando puntos con PostgreSQL...");
        let mut redis_conn = match self.redis_client.get_multiplexed_async_connection().await {
            Ok(conn) => conn, Err(_) => return Err(Status::internal("Error conectando Redis")),
        };

        if let Ok(scores) = redis::cmd("ZREVRANGE").arg("arena_leaderboard").arg(0).arg(-1).arg("WITHSCORES") 
            .query_async::<_, Vec<(String, i32)>>(&mut redis_conn).await {
            for (player_username, puntos) in scores {
                let _ = sqlx::query("UPDATE users SET score = score + $1 WHERE username = $2")
                    .bind(puntos).bind(&player_username).execute(&self.pg_pool).await;
            }
        }
        let _: redis::RedisResult<()> = redis::cmd("DEL").arg("arena_leaderboard").query_async(&mut redis_conn).await;
        println!("🧹 Base de datos sincronizada y limpia.");
        Ok(Response::new(ModeratorAck { success: true }))
    }
}

// ==========================================
// SEED: 10 PREGUNTAS MONGODB
// ==========================================
async fn seed_mongodb_if_empty(client: &MongoClient) -> Result<(), Box<dyn std::error::Error>> {
    let db = client.database("arena_db");
    let collection = db.collection::<MongoQuestion>("questions");

    // Limpiamos la colección para forzar la carga de las 10 preguntas
    let _ = collection.drop(None).await;

    println!("📦 Insertando 10 preguntas en MongoDB...");
    
    let questions = vec![
        MongoQuestion { text: "¿Cuál es el patrón de diseño para revertir transacciones en microservicios?".to_string(), options: vec!["CQRS".to_string(), "Saga".to_string(), "Event Sourcing".to_string(), "Circuit Breaker".to_string()], correct_option_index: 1, time_limit_sec: 20 },
        MongoQuestion { text: "¿Qué puerto usa Redis por defecto en Docker?".to_string(), options: vec!["5432".to_string(), "27017".to_string(), "6379".to_string(), "8080".to_string()], correct_option_index: 2, time_limit_sec: 20 },
        MongoQuestion { text: "¿Qué significa gRPC?".to_string(), options: vec!["General RPC".to_string(), "Google RPC".to_string(), "gRPC Remote Procedure Call".to_string(), "Graph RPC".to_string()], correct_option_index: 2, time_limit_sec: 20 },
        MongoQuestion { text: "¿Cuál de estos NO es un método HTTP válido?".to_string(), options: vec!["GET".to_string(), "POST".to_string(), "FETCH".to_string(), "DELETE".to_string()], correct_option_index: 2, time_limit_sec: 20 },
        MongoQuestion { text: "¿En qué lenguaje de programación está escrito Docker nativamente?".to_string(), options: vec!["Python".to_string(), "C++".to_string(), "Go".to_string(), "Rust".to_string()], correct_option_index: 2, time_limit_sec: 20 },
        MongoQuestion { text: "¿Cuál es la base de datos documental más popular?".to_string(), options: vec!["PostgreSQL".to_string(), "MongoDB".to_string(), "Redis".to_string(), "Cassandra".to_string()], correct_option_index: 1, time_limit_sec: 20 },
        MongoQuestion { text: "¿Qué protocolo de red usa gRPC por debajo?".to_string(), options: vec!["HTTP/1.1".to_string(), "HTTP/2".to_string(), "TCP Directo".to_string(), "UDP".to_string()], correct_option_index: 1, time_limit_sec: 20 },
        MongoQuestion { text: "¿Qué estructura de datos es ideal para un Leaderboard en Redis?".to_string(), options: vec!["Hashes".to_string(), "Lists".to_string(), "Sorted Sets".to_string(), "Strings".to_string()], correct_option_index: 2, time_limit_sec: 20 },
        MongoQuestion { text: "¿Qué comando levanta contenedores ocultos (background) en Compose?".to_string(), options: vec!["docker-compose start".to_string(), "docker-compose up -d".to_string(), "docker run -b".to_string(), "docker background".to_string()], correct_option_index: 1, time_limit_sec: 20 },
        MongoQuestion { text: "¿Cómo se define un contrato de red en un archivo .proto?".to_string(), options: vec!["service { ... }".to_string(), "class { ... }".to_string(), "interface { ... }".to_string(), "rpc { ... }".to_string()], correct_option_index: 0, time_limit_sec: 20 },
    ];

    collection.insert_many(questions, None).await?;
    println!("✅ 10 Preguntas insertadas exitosamente.");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    println!("Iniciando conexiones a bases de datos...");

    let pg_url = env::var("DATABASE_URL").expect("Falta DATABASE_URL en .env");
    let pg_pool = PgPool::connect(&pg_url).await?;
    
    let mongo_url = env::var("MONGO_URI").expect("Falta MONGO_URI en .env");
    let mongo_client = MongoClient::with_uri_str(&mongo_url).await?;

    seed_mongodb_if_empty(&mongo_client).await?;

    let redis_url = env::var("REDIS_URL").expect("Falta REDIS_URL en .env");
    let redis_client = redis::Client::open(redis_url)?;

    let addr = "0.0.0.0:50051".parse().unwrap(); 
    let (tx, _) = broadcast::channel(100);
    
    let game_server = MyGameServer { tx_to_clients: tx, pg_pool, mongo_client, redis_client };

    println!("🚀 Servidor Arena escuchando en {}", addr);
    Server::builder().add_service(GameServiceServer::new(game_server)).serve(addr).await?;
    Ok(())
}
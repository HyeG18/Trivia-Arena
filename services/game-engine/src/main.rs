use tonic::{transport::Server, Request, Response, Status, Streaming};
use tokio::sync::{broadcast, mpsc};
use tokio_stream::wrappers::ReceiverStream;

// IMPORTACIONES PARA LAS BASES DE DATOS
use dotenvy::dotenv;
use std::env;
use sqlx::postgres::PgPool;
use mongodb::Client as MongoClient;
use redis::Client as RedisClient;
use serde::{Deserialize, Serialize}; // <-- NUEVO: Para convertir de Rust a MongoDB (BSON)

pub mod game {
    tonic::include_proto!("arena.game"); 
}

use game::game_service_server::{GameService, GameServiceServer};
use game::{
    ClientMessage, ServerMessage, QuestionPayload, ModeratorAck, 
    EmojiRequest, EmojiAck, ForceEndRequest
}; 

// ==========================================
// 1. MODELO DE DATOS MONGODB
// ==========================================
#[derive(Debug, Serialize, Deserialize)]
pub struct MongoQuestion {
    pub text: String,
    pub options: Vec<String>,
    pub correct_option_index: i32, // 0=A, 1=B, 2=C, 3=D
    pub time_limit_sec: i32,
}

#[derive(Debug)]
pub struct MyGameServer {
    tx_to_clients: broadcast::Sender<ServerMessage>,
    #[allow(dead_code)]
    pg_pool: PgPool,
    mongo_client: MongoClient, // Le quitamos el allow(dead_code) porque ya la vamos a usar
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
        // ¡NUEVO! Clonamos la conexión de Mongo para usarla en el hilo de fondo
        let mongo_client = self.mongo_client.clone(); 

        tokio::spawn(async move {
            let mut redis_conn = match redis_client.get_multiplexed_async_connection().await {
                Ok(conn) => conn,
                Err(e) => { eprintln!("Error conectando a Redis: {}", e); return; }
            };

            while let Ok(Some(message)) = in_stream.message().await {
                // ¡NUEVO! Extraemos los datos REALES del mensaje de gRPC
                if let Some(game::client_message::Payload::Answer(player_response)) = message.payload {
                    let user_id = player_response.user_id;
                    let user_answer = player_response.answer;
                    
                    println!("🎮 Respuesta recibida de {}: {}", user_id, user_answer);

                    // ==========================================
                    // 1. VALIDACIÓN CON MONGODB
                    // ==========================================
                    let db = mongo_client.database("arena_db");
                    let collection = db.collection::<MongoQuestion>("questions");
                    
                    let mut puntos_ganados = 0;
                    let mut es_correcta = false;

                    // Buscamos la pregunta en Mongo (tomamos la primera para la prueba)
                    if let Ok(Some(question)) = collection.find_one(None, None).await {
                        // Buscamos el texto exacto de la opción correcta
                        let correct_text = &question.options[question.correct_option_index as usize];
                        
                        // ¿Lo que envió el jugador coincide con la base de datos?
                        if user_answer == *correct_text {
                            // CÁLCULO DE PUNTAJE DINÁMICO
                            let time_limit_ms = question.time_limit_sec * 1000;
                            let response_time = player_response.response_time_ms;
                            
                            // Si respondió después del límite (por lag), le damos el puntaje mínimo
                            if response_time >= time_limit_ms {
                                puntos_ganados = 300; 
                            } else {
                                // Fórmula: 1500 * (Tiempo Restante / Tiempo Total)
                                let tiempo_restante = time_limit_ms - response_time;
                                let calc = (1500.0 * (tiempo_restante as f64 / time_limit_ms as f64)) as i32;
                                
                                // Garantizamos que mínimo gane 300 puntos por acertar
                                puntos_ganados = std::cmp::max(300, calc);
                            }
                            
                            es_correcta = true;
                            println!("✅ ¡Respuesta correcta de {}!", user_id);
                        } else {
                            println!("❌ {} respondió incorrectamente. Esperaba: {}", user_id, correct_text);
                        }
                    }

                    // ==========================================
                    // 2. GUARDAR EN REDIS Y OBTENER TOP 5
                    // ==========================================
                    // ZINCRBY guardará los puntos (si es incorrecta sumará 0, pero lo mantendrá en el ranking)
                    let result: Result<(), redis::RedisError> = redis::cmd("ZINCRBY")
                        .arg("arena_leaderboard")
                        .arg(puntos_ganados)
                        .arg(&user_id)
                        .query_async(&mut redis_conn)
                        .await;

                    if result.is_ok() {
                        let top_5_result: Result<Vec<(String, i32)>, redis::RedisError> = redis::cmd("ZREVRANGE")
                            .arg("arena_leaderboard").arg(0).arg(4).arg("WITHSCORES")
                            .query_async(&mut redis_conn).await;
                            
                        if let Ok(top_5) = top_5_result {
                            let mut top_players = Vec::new();
                            
                            for (index, (username, score)) in top_5.into_iter().enumerate() {
                                // Determinamos si esta fila de la tabla debe pintar verde o rojo en Java
                                let is_this_player = username == user_id;
                                let is_correct = if is_this_player { es_correcta } else { true }; 

                                top_players.push(game::PlayerScore {
                                    username,
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

    async fn send_emoji(&self, request: Request<EmojiRequest>) -> Result<Response<EmojiAck>, Status> {
        let data = request.into_inner();
        println!("🚀 Emoji recibido del jugador [{}]: {}", data.user_id, data.emoji_code);
        Ok(Response::new(EmojiAck { received: true }))
    }

    // ==========================================
    // 3. ACTUALIZACIÓN: LANZAR PREGUNTA DESDE MONGO
    // ==========================================
    async fn launch_question(&self, _request: Request<QuestionPayload>) -> Result<Response<ModeratorAck>, Status> {
        // Conectamos a la colección de preguntas
        let db = self.mongo_client.database("arena_db");
        let collection = db.collection::<MongoQuestion>("questions");

        // Buscamos la primera pregunta que exista en la base de datos (solo para probar)
        if let Ok(Some(question)) = collection.find_one(None, None).await {
            println!("📚 Pregunta obtenida de MongoDB: {}", question.text);

            // Empaquetamos la pregunta según el contrato .proto
            let payload = QuestionPayload {
                text: question.text,
                options: question.options,
                time_limit_sec: question.time_limit_sec,
            };

            // Disparamos la pregunta a todos los jugadores conectados
            let msg = ServerMessage { event: Some(game::server_message::Event::NewQuestion(payload)) };
            let _ = self.tx_to_clients.send(msg);

            Ok(Response::new(ModeratorAck { success: true }))
        } else {
            println!("⚠️ No hay preguntas en MongoDB.");
            Ok(Response::new(ModeratorAck { success: false }))
        }
    }

    async fn force_end_timer(&self, _request: Request<ForceEndRequest>) -> Result<Response<ModeratorAck>, Status> {
        Ok(Response::new(ModeratorAck { success: true }))
    }
}

// ==========================================
// 2. FUNCIÓN PARA POBLAR LA BASE DE DATOS
// ==========================================
async fn seed_mongodb_if_empty(client: &MongoClient) -> Result<(), Box<dyn std::error::Error>> {
    let db = client.database("arena_db");
    let collection = db.collection::<MongoQuestion>("questions");

    // Revisamos cuántas preguntas hay
    let count = collection.count_documents(None, None).await?;
    
    if count == 0 {
        println!("📦 MongoDB está vacío. Insertando pregunta del diseñador UI...");
        
        // ¡La pregunta exacta de tu diseño!
        let test_question = MongoQuestion {
            text: "¿Cuál es el patrón de diseño que permite revertir transacciones en múltiples microservicios?".to_string(),
            options: vec![
                "CQRS".to_string(),
                "Saga".to_string(),
                "Event Sourcing".to_string(),
                "Circuit Breaker".to_string()
            ],
            correct_option_index: 1, // La opción B (Saga)
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

    // EJECUTAMOS NUESTRO SEEDER
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
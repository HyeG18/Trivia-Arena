# 🎮 Trivia Arena

> Un sistema distribuido y nativo de la nube para juegos de trivia multijugador en tiempo real, diseñado con arquitectura de microservicios y persistencia políglota.

Trivia Arena es una plataforma lúdica interactiva (al estilo Kahoot!) desarrollada como proyecto de Computación en la Nube y Sistemas Distribuidos. Está diseñada desde cero para superar las limitaciones de las arquitecturas monolíticas REST tradicionales, utilizando **Rust** y **gRPC (sobre HTTP/2)** para lograr transmisiones de datos bidireccionales, asíncronas y de latencia ultra baja.

## ✨ Características Principales

* **🚀 Streaming Bidireccional en Tiempo Real:** Las preguntas, el temporizador y el Leaderboard se actualizan en milisegundos gracias a streams persistentes gRPC.
* **🛡️ Tolerancia a Fallos y Resiliencia:** Autodetección de red en los clientes. Si un cliente pierde conexión (partición de red), la interfaz se bloquea y se reconecta automáticamente sin crashear.
* **💾 Persistencia Políglota:** El estado se gestiona de forma óptima asignando cada dato a su motor ideal: **PostgreSQL** (Transaccional), **MongoDB** (Documental) y **Redis** (En Memoria).
* **🧠 Patrones de Sistemas Distribuidos:** Implementación real de *API Gateway, Pub/Sub, Bulkhead, Saga (Transacciones compensatorias)* y *Strategy*.
* **🐳 Contenerización:** Ecosistema completamente orquestado con Docker Compose, con redes internas aisladas.

---

## 🏗️ Arquitectura del Sistema

El ecosistema se compone de los siguientes componentes:

### 1. Capa de Microservicios (Backend en Rust 🦀)
* **API Gateway:** Único punto de entrada expuesto (Puerto 8080). Enruta peticiones y filtra *streams* caídos o corruptos.
* **Auth Service:** Gestiona el registro y *login* de usuarios. Implementa el **Patrón Saga** para asegurar consistencia eventual entre Postgres y Redis (hace *Rollback* si la caché falla).
* **Game Engine:** El núcleo del juego. Usa el **Patrón Pub/Sub** (con `tokio::broadcast`) para distribuir preguntas y reacciones (emojis) a múltiples clientes concurrentes.

### 2. Capa de Bases de Datos
* 🐘 **PostgreSQL:** Almacena usuarios, credenciales hasheadas y puntajes históricos.
* 🍃 **MongoDB:** Almacena el repositorio dinámico de preguntas y sus opciones.
* ⚡ **Redis:** Motor de estado en caliente. Gestiona sesiones temporales y calcula el *Leaderboard* en microsegundos usando estructuras *Sorted Sets*.

### 3. Clientes (Frontend)
* ☕ **Cliente Jugador (Java Swing):** Interfaz gamificada para los participantes. Recibe preguntas, muestra cronómetros y envía reacciones asíncronas.
* 🐍 **Cliente Moderador (Python Tkinter):** Panel de orquestación administrativo. Permite lanzar las preguntas masivamente (`LaunchQuestion`) y forzar el fin del juego (`ForceEndTimer`).

---

## 🛠️ Requisitos Previos

Para ejecutar este proyecto en tu máquina local, necesitarás tener instalado:

* [Docker](https://www.docker.com/products/docker-desktop/) y Docker Compose.
* [Java JDK 11+](https://adoptium.net/) (Para ejecutar el cliente jugador).
* [Python 3.8+](https://www.python.org/downloads/) (Para ejecutar el cliente moderador).

---

## 🚀 Guía de Ejecución

### 1. Levantar la Infraestructura y Microservicios

Clona este repositorio y navega a la carpeta raíz. Luego, levanta los contenedores en segundo plano:

```bash
docker-compose up -d --build
⚠️ Nota para usuarios de Windows (Error de puertos): > Si al levantar Docker recibes un error como An attempt was made to access a socket in a way forbidden by its access permissions (frecuente con el puerto de MongoDB), abre PowerShell como Administrador y ejecuta: net stop winnat seguido de net start winnat. Luego vuelve a ejecutar docker-compose up -d.

Verifica que los 4 contenedores (Gateway, Postgres, Mongo, Redis) estén corriendo:

Bash
docker ps
2. Ejecutar el Cliente del Jugador (Java)
Abre el proyecto Java en tu IDE favorito (IntelliJ IDEA, Eclipse, etc.) o compílalo desde la terminal.

Ejecuta la clase principal (Main o equivalente) que lanza la interfaz Swing.

Puedes abrir múltiples instancias de la aplicación Java para simular a varios jugadores conectándose a la misma partida.

3. Ejecutar el Panel del Moderador (Python)
Se recomienda usar un entorno virtual para las dependencias de gRPC en Python:

Bash
# Navegar a la carpeta del cliente Python
cd clients/python-moderator

# Crear y activar entorno virtual
python -m venv venv
source venv/bin/activate  # En Windows: venv\Scripts\activate

# Instalar dependencias necesarias (grpcio, grpcio-tools)
pip install -r requirements.txt

# Ejecutar el panel
python app.py
4. Flujo de Juego Básico
Abre 2 o 3 clientes Java y registra/inicia sesión con usuarios distintos.

Abre el cliente Python (Moderador).

Desde Python, presiona "Lanzar Pregunta". Observarás cómo todos los clientes Java reciben la pregunta simultáneamente.

Responde desde los clientes Java. El Leaderboard se actualizará en tiempo real.

Desde Python, presiona "Forzar Fin" para guardar los puntajes finales en PostgreSQL de manera asíncrona.

📚 Bibliotecas y Tecnologías Clave
Backend: Rust, Tokio (Asíncrono), Tonic (gRPC), SQLx (Postgres), mongodb (driver), redis-rs.

Frontend: Java (Swing, grpc-java), Python (Tkinter, grpcio).

Protocolo: gRPC, Protocol Buffers (Protobuf), HTTP/2.

✒️ Autores
[Tu Nombre/Usuario de GitHub] - Diseño de Arquitectura, Desarrollo y Despliegue.

Proyecto desarrollado para la cátedra de Computación en la Nube / Sistemas Distribuidos - Semestre 2026.
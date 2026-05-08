import tkinter as tk
from tkinter import messagebox, simpledialog
import socket
import grpc

# Importamos los archivos generados por gRPC
import game_pb2
import game_pb2_grpc

class ModeratorApp:
    def __init__(self, root):
        self.root = root
        self.root.title("Panel de Moderador - Trivia Arena")
        self.root.geometry("450x300")
        self.root.configure(bg="#2c3e50")
        
        # --- AUTO-DETECTAR IP LOCAL ---
        try:
            # Crea un socket temporal para descubrir la IP de la interfaz de red principal
            s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            s.connect(("8.8.8.8", 80))
            local_ip = s.getsockname()[0]
            s.close()
        except Exception:
            local_ip = "127.0.0.1"

        # Mostrar ventana pidiendo la IP (con la local por defecto)
        self.root.withdraw() # Ocultamos la principal momentáneamente
        server_ip = simpledialog.askstring("Conexión", "Ingresa la IP del Servidor Game Engine:", initialvalue=local_ip)
        
        if not server_ip:
            messagebox.showerror("Error", "Se necesita una IP para conectarse al servidor.")
            self.root.destroy()
            return
            
        self.root.deiconify() # Mostramos la principal

        # --- CONFIGURACIÓN gRPC ---
        # Nos conectamos al API Gateway usando la IP proporcionada
        self.channel = grpc.insecure_channel(f'{server_ip}:8080')
        self.stub = game_pb2_grpc.GameServiceStub(self.channel)
        
        # --- INTERFAZ GRÁFICA (UI) ---
        self.title_label = tk.Label(
            root, text="🎮 Panel de Control", 
            font=("Helvetica", 18, "bold"), bg="#2c3e50", fg="white"
        )
        self.title_label.pack(pady=30)
        
        # Botón para lanzar la pregunta
        self.btn_launch = tk.Button(
            root, text="🚀 Lanzar Siguiente Pregunta", 
            font=("Helvetica", 12, "bold"), bg="#27ae60", fg="white",
            activebackground="#2ecc71", cursor="hand2",
            command=self.launch_question
        )
        self.btn_launch.pack(fill='x', padx=50, pady=10, ipady=5)
        
        # Botón para finalizar y sincronizar
        self.btn_end = tk.Button(
            root, text="🛑 Finalizar Partida (Guardar Puntos)", 
            font=("Helvetica", 12, "bold"), bg="#c0392b", fg="white",
            activebackground="#e74c3c", cursor="hand2",
            command=self.force_end
        )
        self.btn_end.pack(fill='x', padx=50, pady=10, ipady=5)

    # --- LÓGICA DE NEGOCIO ---
    def launch_question(self):
        try:
            req = game_pb2.QuestionPayload(text="Siguiente", options=[], time_limit_sec=0)
            response = self.stub.LaunchQuestion(req)
            
            if response.success:
                messagebox.showinfo("Éxito", "¡Preguntas automáticas lanzadas a los jugadores conectados!")
            else:
                messagebox.showwarning("Aviso", "No se pudo lanzar. ¿Quizás MongoDB está vacío?")
                
        except grpc.RpcError as e:
            messagebox.showerror("Error de Servidor", f"El backend no responde o se perdió la conexión.\n\nRevisa tu red.\n\nDetalles técnicos: {e.details()}")

    def force_end(self):
        if not messagebox.askyesno("Confirmar", "¿Estás seguro de finalizar la partida y guardar todos los puntos en PostgreSQL?"):
            return

        try:
            req = game_pb2.ForceEndRequest(moderator_id="admin_python")
            response = self.stub.ForceEndTimer(req)
            
            if response.success:
                messagebox.showinfo("Partida Finalizada", "El tablero se ha limpiado y los puntos se han guardado en la Base de Datos.")
            else:
                messagebox.showerror("Error", "Ocurrió un problema finalizando la partida.")
                
        except grpc.RpcError as e:
            messagebox.showerror("Error de Servidor", f"El backend no responde o se perdió la conexión.\n{e.details()}")

if __name__ == "__main__":
    ventana_principal = tk.Tk()
    app = ModeratorApp(ventana_principal)
    ventana_principal.mainloop()
package com.trivia;

import arena.game.GameServiceGrpc;
import arena.game.Game.*;
import arena.user.UserServiceGrpc;
import arena.user.User.*;

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.stub.StreamObserver;

import javax.swing.*;
import javax.swing.border.EmptyBorder;
import java.awt.*;
import java.awt.event.ActionEvent;
import java.awt.event.ActionListener;
import java.net.InetAddress;
import java.net.NetworkInterface;
import java.util.Enumeration;

public class TriviaClientApp extends JFrame {

    private ManagedChannel channel;
    private UserServiceGrpc.UserServiceBlockingStub authStub;
    private GameServiceGrpc.GameServiceStub gameStub;
    private GameServiceGrpc.GameServiceBlockingStub gameBlockingStub;
    private StreamObserver<ClientMessage> requestObserver;

    private String userId;
    private String username;
    private long questionStartTime;
    private boolean isReconnecting = false; // Bandera para evitar múltiples hilos de reconexión

    private JPanel mainPanel;
    private CardLayout cardLayout;

    private JTextField ipField; 
    private JTextField userField;
    private JPasswordField passField;
    private JLabel loginStatusLabel;

    private JLabel questionLabel;
    private JLabel emojiDisplayLabel;
    private JPanel optionsPanel;
    private JTextArea leaderboardArea;
    
    private JProgressBar timerBar;
    private Timer questionTimer;

    // --- NUEVO: Paleta de colores gamificada ---
    private final Color BG_PURPLE = new Color(138, 43, 226); 
    private final Color[] BTN_COLORS = {
        new Color(231, 76, 60),  // Rojo
        new Color(52, 152, 219), // Azul
        new Color(241, 196, 15), // Amarillo
        new Color(46, 204, 113)  // Verde
    };

    public TriviaClientApp() {
        setTitle("Trivia Arena - Jugador");
        setSize(800, 700); // Ampliado para mejor visualización de los botones
        setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);

        cardLayout = new CardLayout();
        mainPanel = new JPanel(cardLayout);

        mainPanel.add(createLoginPanel(), "LOGIN");
        mainPanel.add(createGamePanel(), "GAME");

        add(mainPanel);

        // --- PANTALLA DE DESCONEXIÓN (RESILIENCIA) ---
        JPanel glassPane = new JPanel(new BorderLayout());
        glassPane.setBackground(new Color(0, 0, 0, 200)); // Negro semitransparente
        JLabel loadLabel = new JLabel("📡 Se perdió la conexión. Intentando reconectar...", SwingConstants.CENTER);
        loadLabel.setFont(new Font("Arial", Font.BOLD, 22));
        loadLabel.setForeground(Color.WHITE);
        glassPane.add(loadLabel, BorderLayout.CENTER);
        setGlassPane(glassPane);

        cardLayout.show(mainPanel, "LOGIN");
    }

    private JPanel createLoginPanel() {
        JPanel panel = new JPanel(new GridBagLayout());
        panel.setBackground(BG_PURPLE); // Estilo: Fondo morado
        GridBagConstraints gbc = new GridBagConstraints();
        gbc.insets = new Insets(10, 10, 10, 10);
        gbc.gridx = 0; gbc.gridy = 0; gbc.gridwidth = 2;

        JLabel title = new JLabel("Bienvenido a Trivia Arena");
        title.setForeground(Color.WHITE);
        title.setFont(new Font("Arial", Font.BOLD, 28)); // Letra más grande
        panel.add(title, gbc);

        // --- AUTO-DETECTAR IP LOCAL ---
        String localIp = getLocalIP();

        gbc.gridwidth = 1; gbc.gridy = 1;
        JLabel ipLabel = new JLabel("IP del Servidor:");
        ipLabel.setForeground(Color.WHITE);
        ipLabel.setFont(new Font("Arial", Font.BOLD, 14));
        panel.add(ipLabel, gbc);

        gbc.gridx = 1;
        ipField = new JTextField(localIp, 15);
        panel.add(ipField, gbc);

        gbc.gridx = 0; gbc.gridy = 2;
        JLabel userLabel = new JLabel("Usuario:");
        userLabel.setForeground(Color.WHITE);
        userLabel.setFont(new Font("Arial", Font.BOLD, 14));
        panel.add(userLabel, gbc);

        gbc.gridx = 1;
        userField = new JTextField(15);
        panel.add(userField, gbc);

        gbc.gridx = 0; gbc.gridy = 3;
        JLabel passLabel = new JLabel("Contraseña:");
        passLabel.setForeground(Color.WHITE);
        passLabel.setFont(new Font("Arial", Font.BOLD, 14));
        panel.add(passLabel, gbc);

        gbc.gridx = 1;
        passField = new JPasswordField(15);
        panel.add(passField, gbc);

        gbc.gridx = 0; gbc.gridy = 4; gbc.gridwidth = 2;
        JButton loginBtn = new JButton("Entrar a la Arena");
        loginBtn.setFont(new Font("Arial", Font.BOLD, 16));
        loginBtn.setBackground(new Color(39, 174, 96));
        loginBtn.setForeground(Color.WHITE);
        loginBtn.setFocusPainted(false);
        loginBtn.addActionListener(e -> attemptLogin());
        panel.add(loginBtn, gbc);

        gbc.gridy = 5;
        loginStatusLabel = new JLabel("");
        loginStatusLabel.setForeground(Color.YELLOW);
        panel.add(loginStatusLabel, gbc);

        return panel;
    }

    private String getLocalIP() {
        try (java.net.DatagramSocket socket = new java.net.DatagramSocket()) {
            // Simulamos una conexión hacia afuera (el DNS de Google)
            socket.connect(java.net.InetAddress.getByName("8.8.8.8"), 80);
            // Le preguntamos al socket qué IP local usó para esa ruta
            return socket.getLocalAddress().getHostAddress();
        } catch (Exception e) {
            return "127.0.0.1";
        }
    }

    private void attemptLogin() {
        String serverIp = ipField.getText().trim();
        username = userField.getText();
        String password = new String(passField.getPassword());

        try {
            channel = ManagedChannelBuilder.forAddress(serverIp, 8080)
                    .usePlaintext()
                    .build();
            authStub = UserServiceGrpc.newBlockingStub(channel);
            gameStub = GameServiceGrpc.newStub(channel);
            gameBlockingStub = GameServiceGrpc.newBlockingStub(channel);

            JoinRequest request = JoinRequest.newBuilder()
                    .setUsername(username)
                    .setPassword(password)
                    .build();

            JoinResponse response = authStub.joinArena(request);

            if (response.getSuccess()) {
                userId = response.getUserId();
                cardLayout.show(mainPanel, "GAME");
                connectToGameStream();
            } else {
                loginStatusLabel.setText(response.getMessage());
            }
        } catch (Exception ex) {
            loginStatusLabel.setText("Error: Servidor no encontrado en " + serverIp);
        }
    }

    private JPanel createGamePanel() {
        JPanel panel = new JPanel(new BorderLayout(15, 15));
        panel.setBackground(BG_PURPLE); // Estilo: Fondo morado
        panel.setBorder(new EmptyBorder(20, 20, 20, 20));

        JPanel topPanel = new JPanel(new BorderLayout(10, 15));
        topPanel.setOpaque(false); // Estilo: Transparente
        
        timerBar = new JProgressBar(0, 20000);
        timerBar.setValue(20000);
        timerBar.setStringPainted(true);
        timerBar.setString("Esperando...");
        timerBar.setForeground(new Color(46, 204, 113)); 
        timerBar.setBackground(Color.WHITE);
        topPanel.add(timerBar, BorderLayout.NORTH);

        // Estilo: Caja de pregunta blanca
        JPanel questionBox = new JPanel(new BorderLayout());
        questionBox.setBackground(Color.WHITE);
        questionBox.setBorder(BorderFactory.createCompoundBorder(
                BorderFactory.createLineBorder(Color.LIGHT_GRAY, 2, true),
                new EmptyBorder(30, 20, 30, 20)
        ));

        questionLabel = new JLabel("Esperando a que el moderador inicie...", SwingConstants.CENTER);
        questionLabel.setFont(new Font("Arial", Font.BOLD, 22)); // Letra más grande
        questionBox.add(questionLabel, BorderLayout.CENTER);
        topPanel.add(questionBox, BorderLayout.CENTER);

        emojiDisplayLabel = new JLabel(" ", SwingConstants.CENTER);
        emojiDisplayLabel.setFont(new Font("Segoe UI Emoji", Font.PLAIN, 45));
        topPanel.add(emojiDisplayLabel, BorderLayout.SOUTH);

        panel.add(topPanel, BorderLayout.NORTH);

        optionsPanel = new JPanel(new GridLayout(2, 2, 15, 15));
        optionsPanel.setOpaque(false); // Estilo: Transparente
        panel.add(optionsPanel, BorderLayout.CENTER);

        JPanel southPanel = new JPanel(new BorderLayout(10, 10));
        southPanel.setOpaque(false); // Estilo: Transparente

        leaderboardArea = new JTextArea(6, 30);
        leaderboardArea.setEditable(false);
        leaderboardArea.setFont(new Font("Monospaced", Font.BOLD, 14));
        leaderboardArea.setBorder(new EmptyBorder(5, 5, 5, 5));
        southPanel.add(new JScrollPane(leaderboardArea), BorderLayout.CENTER);

        JPanel emojiButtonsPanel = new JPanel(new FlowLayout());
        emojiButtonsPanel.setOpaque(false); // Estilo: Transparente

        JLabel reactionLabel = new JLabel("Reacciones: ");
        reactionLabel.setForeground(Color.WHITE);
        reactionLabel.setFont(new Font("Arial", Font.BOLD, 14));
        emojiButtonsPanel.add(reactionLabel);

        String[] emojis = { "🚀", "😂", "😭" };
        for (String em : emojis) {
            JButton btn = new JButton(em);
            btn.setFont(new Font("Segoe UI Emoji", Font.PLAIN, 24));
            btn.setFocusPainted(false);
            btn.setBackground(Color.WHITE);
            btn.addActionListener(e -> sendEmojiToServer(em));
            emojiButtonsPanel.add(btn);
        }

        southPanel.add(emojiButtonsPanel, BorderLayout.SOUTH);
        panel.add(southPanel, BorderLayout.SOUTH);

        return panel;
    }

    private void sendEmojiToServer(String emojiCode) {
        try {
            EmojiRequest req = EmojiRequest.newBuilder().setUserId(userId).setEmojiCode(emojiCode).build();
            gameBlockingStub.sendEmoji(req);
        } catch (Exception ex) {
            System.err.println("Error: " + ex.getMessage());
        }
    }

    private void connectToGameStream() {
        requestObserver = gameStub.playStream(new StreamObserver<ServerMessage>() {
            @Override
            public void onNext(ServerMessage msg) {
                SwingUtilities.invokeLater(() -> {
                    if (msg.hasNewQuestion()) {
                        handleNewQuestion(msg.getNewQuestion());
                    } else if (msg.hasLeaderboard()) {
                        handleLeaderboard(msg.getLeaderboard());
                    } else if (msg.hasEmojiBroadcast()) {
                        showIncomingEmoji(msg.getEmojiBroadcast().getEmojiCode());
                    }
                });
            }

            @Override 
            public void onError(Throwable t) { 
                SwingUtilities.invokeLater(() -> {
                    if (questionTimer != null) questionTimer.stop(); 
                    getGlassPane().setVisible(true); 
                    attemptReconnect();
                });
            }
            
            @Override 
            public void onCompleted() { }
        });

        PlayerResponse ping = PlayerResponse.newBuilder().setUserId(userId).setAnswer("").setResponseTimeMs(0).build();
        requestObserver.onNext(ClientMessage.newBuilder().setAnswer(ping).build());
    }

    private void attemptReconnect() {
        if (isReconnecting) return;
        isReconnecting = true;

        new Thread(() -> {
            boolean connected = false;
            while (!connected) {
                try {
                    Thread.sleep(3000); 
                    
                    gameBlockingStub.sendEmoji(EmojiRequest.newBuilder().setUserId("ping").setEmojiCode("ping").build());
                    
                    connectToGameStream(); 
                    connected = true;
                    isReconnecting = false;
                    
                    SwingUtilities.invokeLater(() -> {
                        getGlassPane().setVisible(false); 
                        if (questionTimer != null && timerBar.getValue() > 0) {
                            questionTimer.start(); 
                        }
                    });
                } catch (Exception e) {
                    System.out.println("Sigue sin conexión... reintentando.");
                }
            }
        }).start();
    }

    private void showIncomingEmoji(String emojiCode) {
        emojiDisplayLabel.setText(emojiCode);
        Timer timer = new Timer(2000, e -> emojiDisplayLabel.setText(" "));
        timer.setRepeats(false);
        timer.start();
    }

    private void handleNewQuestion(QuestionPayload q) {
        // Estilo: Centrado HTML para la pregunta
        questionLabel.setText("<html><div style='text-align: center;'>" + q.getText() + "</div></html>");
        optionsPanel.removeAll();
        
        if (questionTimer != null) {
            questionTimer.stop();
        }

        if (q.getOptionsList().isEmpty()) {
            timerBar.setValue(0);
            timerBar.setString("PARTIDA FINALIZADA");
            optionsPanel.revalidate();
            optionsPanel.repaint();
            return;
        }

        questionStartTime = System.currentTimeMillis();
        int timeLimitMs = q.getTimeLimitSec() * 1000;
        
        timerBar.setMaximum(timeLimitMs);
        timerBar.setValue(timeLimitMs);
        timerBar.setForeground(new Color(46, 204, 113)); 

        questionTimer = new Timer(100, e -> {
            long elapsed = System.currentTimeMillis() - questionStartTime;
            int remaining = timeLimitMs - (int) elapsed;

            if (remaining <= 0) {
                questionTimer.stop();
                timerBar.setValue(0);
                timerBar.setString("¡Tiempo Agotado!");
                for (Component c : optionsPanel.getComponents()) {
                    c.setEnabled(false);
                }
            } else {
                timerBar.setValue(remaining);
                timerBar.setString((remaining / 1000) + " segundos");
                if (remaining <= 5000) {
                    timerBar.setForeground(Color.RED);
                }
            }
        });
        questionTimer.start();

        // Estilo: Botones de colores
        int colorIndex = 0;
        for (String optionText : q.getOptionsList()) {
            JButton btn = new JButton(optionText);
            btn.setFont(new Font("Arial", Font.BOLD, 18));
            btn.setForeground(Color.WHITE);
            
            if (colorIndex < BTN_COLORS.length) {
                btn.setBackground(BTN_COLORS[colorIndex]);
                btn.setOpaque(true);
            }
            
            btn.setFocusPainted(false);
            btn.addActionListener(e -> sendAnswer(optionText));
            optionsPanel.add(btn);
            colorIndex++;
        }

        optionsPanel.revalidate();
        optionsPanel.repaint();
    }

    private void sendAnswer(String answerSelected) {
        int responseTimeMs = (int) (System.currentTimeMillis() - questionStartTime);
        
        for (Component c : optionsPanel.getComponents()) {
            c.setEnabled(false);
        }

        PlayerResponse response = PlayerResponse.newBuilder()
                .setUserId(userId)
                .setAnswer(answerSelected)
                .setResponseTimeMs(responseTimeMs)
                .build();

        requestObserver.onNext(ClientMessage.newBuilder().setAnswer(response).build());
    }

    private void handleLeaderboard(LeaderboardUpdate board) {
        StringBuilder sb = new StringBuilder();
        sb.append("🏆 LEADERBOARD 🏆\n");
        sb.append("--------------------\n");
        for (PlayerScore ps : board.getTopPlayersList()) {
            String icon = ps.getLastAnswerCorrect() ? "✅" : "❌";
            sb.append(String.format("%d. %s - %d pts %s\n", ps.getRank(), ps.getUsername(), ps.getScore(), icon));
        }
        leaderboardArea.setText(sb.toString());
    }

    public static void main(String[] args) {
        // Estilo: Forzar renderizado de colores en Windows/Mac
        try {
            UIManager.setLookAndFeel(UIManager.getCrossPlatformLookAndFeelClassName());
        } catch (Exception e) {}

        SwingUtilities.invokeLater(() -> {
            TriviaClientApp app = new TriviaClientApp();
            app.setVisible(true);
        });
    }
}
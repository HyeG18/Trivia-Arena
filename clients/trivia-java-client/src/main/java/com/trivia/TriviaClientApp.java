package com.trivia;

import arena.game.GameServiceGrpc;
import arena.game.Game.*;
import arena.user.UserServiceGrpc;
import arena.user.User.*;

import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.stub.StreamObserver;

import javax.swing.*;
import java.awt.*;
import java.awt.event.ActionEvent;
import java.awt.event.ActionListener;

public class TriviaClientApp extends JFrame {

    private ManagedChannel channel;
    private UserServiceGrpc.UserServiceBlockingStub authStub;
    private GameServiceGrpc.GameServiceStub gameStub;
    private GameServiceGrpc.GameServiceBlockingStub gameBlockingStub;
    private StreamObserver<ClientMessage> requestObserver;

    private String userId;
    private String username;
    private long questionStartTime;

    private JPanel mainPanel;
    private CardLayout cardLayout;

    private JTextField userField;
    private JPasswordField passField;
    private JLabel loginStatusLabel;

    private JLabel questionLabel;
    private JLabel emojiDisplayLabel;
    private JPanel optionsPanel;
    private JTextArea leaderboardArea;
    
    // NUEVO: Componentes del Temporizador
    private JProgressBar timerBar;
    private Timer questionTimer;

    public TriviaClientApp() {
        setTitle("Trivia Arena - Jugador");
        setSize(650, 650); // Un poco más alto para que quepa la barra
        setDefaultCloseOperation(JFrame.EXIT_ON_CLOSE);

        channel = ManagedChannelBuilder.forAddress("localhost", 8080)
                .usePlaintext()
                .build();
        authStub = UserServiceGrpc.newBlockingStub(channel);
        gameStub = GameServiceGrpc.newStub(channel);
        gameBlockingStub = GameServiceGrpc.newBlockingStub(channel);

        cardLayout = new CardLayout();
        mainPanel = new JPanel(cardLayout);

        mainPanel.add(createLoginPanel(), "LOGIN");
        mainPanel.add(createGamePanel(), "GAME");

        add(mainPanel);
        cardLayout.show(mainPanel, "LOGIN");
    }

    private JPanel createLoginPanel() {
        JPanel panel = new JPanel(new GridBagLayout());
        panel.setBackground(new Color(44, 62, 80));
        GridBagConstraints gbc = new GridBagConstraints();
        gbc.insets = new Insets(10, 10, 10, 10);
        gbc.gridx = 0; gbc.gridy = 0; gbc.gridwidth = 2;

        JLabel title = new JLabel("Bienvenido a Trivia Arena");
        title.setForeground(Color.WHITE);
        title.setFont(new Font("Arial", Font.BOLD, 24));
        panel.add(title, gbc);

        gbc.gridwidth = 1; gbc.gridy = 1;
        JLabel userLabel = new JLabel("Usuario:");
        userLabel.setForeground(Color.WHITE);
        panel.add(userLabel, gbc);

        gbc.gridx = 1;
        userField = new JTextField(15);
        panel.add(userField, gbc);

        gbc.gridx = 0; gbc.gridy = 2;
        JLabel passLabel = new JLabel("Contraseña:");
        passLabel.setForeground(Color.WHITE);
        panel.add(passLabel, gbc);

        gbc.gridx = 1;
        passField = new JPasswordField(15);
        panel.add(passField, gbc);

        gbc.gridx = 0; gbc.gridy = 3; gbc.gridwidth = 2;
        JButton loginBtn = new JButton("Entrar a la Arena");
        loginBtn.setBackground(new Color(39, 174, 96));
        loginBtn.setForeground(Color.WHITE);
        loginBtn.addActionListener(e -> attemptLogin());
        panel.add(loginBtn, gbc);

        gbc.gridy = 4;
        loginStatusLabel = new JLabel("");
        loginStatusLabel.setForeground(Color.YELLOW);
        panel.add(loginStatusLabel, gbc);

        return panel;
    }

    private void attemptLogin() {
        username = userField.getText();
        String password = new String(passField.getPassword());

        try {
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
            loginStatusLabel.setText("Error conectando al servidor.");
        }
    }

    private JPanel createGamePanel() {
        JPanel panel = new JPanel(new BorderLayout(10, 10));
        panel.setBorder(BorderFactory.createEmptyBorder(20, 20, 20, 20));

        // Panel Superior: Barra de tiempo, Pregunta y Emojis
        JPanel topPanel = new JPanel(new BorderLayout(5, 5));
        
        // NUEVO: Barra de progreso de tiempo
        timerBar = new JProgressBar(0, 20000);
        timerBar.setValue(20000);
        timerBar.setStringPainted(true);
        timerBar.setString("Esperando...");
        timerBar.setForeground(new Color(46, 204, 113)); // Verde
        topPanel.add(timerBar, BorderLayout.NORTH);

        questionLabel = new JLabel("Esperando a que el moderador inicie...", SwingConstants.CENTER);
        questionLabel.setFont(new Font("Arial", Font.BOLD, 18));
        topPanel.add(questionLabel, BorderLayout.CENTER);

        emojiDisplayLabel = new JLabel(" ", SwingConstants.CENTER);
        emojiDisplayLabel.setFont(new Font("Segoe UI Emoji", Font.PLAIN, 40));
        topPanel.add(emojiDisplayLabel, BorderLayout.SOUTH);

        panel.add(topPanel, BorderLayout.NORTH);

        // Opciones
        optionsPanel = new JPanel(new GridLayout(2, 2, 10, 10));
        panel.add(optionsPanel, BorderLayout.CENTER);

        // Panel Inferior: Leaderboard y Reacciones
        JPanel southPanel = new JPanel(new BorderLayout(5, 5));

        leaderboardArea = new JTextArea(8, 30);
        leaderboardArea.setEditable(false);
        leaderboardArea.setFont(new Font("Monospaced", Font.PLAIN, 14));
        southPanel.add(new JScrollPane(leaderboardArea), BorderLayout.CENTER);

        JPanel emojiButtonsPanel = new JPanel(new FlowLayout());
        emojiButtonsPanel.setBorder(BorderFactory.createTitledBorder("Reacciones"));

        String[] emojis = { "🚀", "😂", "😭" };
        for (String em : emojis) {
            JButton btn = new JButton(em);
            btn.setFont(new Font("Segoe UI Emoji", Font.PLAIN, 24));
            btn.setFocusPainted(false);
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
            @Override public void onError(Throwable t) { }
            @Override public void onCompleted() { }
        });

        PlayerResponse ping = PlayerResponse.newBuilder().setUserId(userId).setAnswer("").setResponseTimeMs(0).build();
        requestObserver.onNext(ClientMessage.newBuilder().setAnswer(ping).build());
    }

    private void showIncomingEmoji(String emojiCode) {
        emojiDisplayLabel.setText(emojiCode);
        Timer timer = new Timer(2000, e -> emojiDisplayLabel.setText(" "));
        timer.setRepeats(false);
        timer.start();
    }

    private void handleNewQuestion(QuestionPayload q) {
        questionLabel.setText(q.getText());
        optionsPanel.removeAll();
        
        if (questionTimer != null) {
            questionTimer.stop();
        }

        // Si la pregunta no tiene opciones, significa que la partida terminó
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
        timerBar.setForeground(new Color(46, 204, 113)); // Verde inicial

        // Bucle del temporizador visual cada 100ms
        questionTimer = new Timer(100, e -> {
            long elapsed = System.currentTimeMillis() - questionStartTime;
            int remaining = timeLimitMs - (int) elapsed;

            if (remaining <= 0) {
                questionTimer.stop();
                timerBar.setValue(0);
                timerBar.setString("¡Tiempo Agotado!");
                // Bloquear los botones si se acabó el tiempo
                for (Component c : optionsPanel.getComponents()) {
                    c.setEnabled(false);
                }
            } else {
                timerBar.setValue(remaining);
                timerBar.setString((remaining / 1000) + " segundos");
                // Cambiar a color rojo si quedan menos de 5 segundos
                if (remaining <= 5000) {
                    timerBar.setForeground(Color.RED);
                }
            }
        });
        questionTimer.start();

        for (String optionText : q.getOptionsList()) {
            JButton btn = new JButton(optionText);
            btn.setFont(new Font("Arial", Font.BOLD, 14));
            btn.addActionListener(e -> sendAnswer(optionText));
            optionsPanel.add(btn);
        }

        optionsPanel.revalidate();
        optionsPanel.repaint();
    }

    private void sendAnswer(String answerSelected) {
        int responseTimeMs = (int) (System.currentTimeMillis() - questionStartTime);
        
        // Bloquear todos los botones para que no responda dos veces
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
        SwingUtilities.invokeLater(() -> {
            TriviaClientApp app = new TriviaClientApp();
            app.setVisible(true);
        });
    }
}
package com.arenadepreguntas.client.controller;

import com.arenadepreguntas.client.GrpcClientService;
import com.arenadepreguntas.client.SessionData;
import com.arenadepreguntas.grpc.game.LeaderboardUpdate;
import com.arenadepreguntas.grpc.game.QuestionPayload;
import com.arenadepreguntas.grpc.game.ServerMessage;

import javafx.animation.FadeTransition;
import javafx.animation.Interpolator;
import javafx.animation.Timeline;
import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.layout.StackPane;
import javafx.util.Duration;

import java.util.List;

/**
 * Arena/game screen controller. Handles real-time game flow from the server.
 * Receives server events (NewQuestion, LeaderboardUpdate) and updates the UI
 * accordingly.
 */
public class ArenaController {

    // ========================================================================
    // FXML bindings
    // ========================================================================

    @FXML
    private StackPane arenaRoot;
    @FXML
    private ProgressBar timerBar;
    @FXML
    private Label questionMeta;
    @FXML
    private Label questionText;
    @FXML
    private Button answerBtnA;
    @FXML
    private Button answerBtnB;
    @FXML
    private Button answerBtnC;
    @FXML
    private Button answerBtnD;
    @FXML
    private Label headerUsername;
    @FXML
    private Label scoreLabel;
    @FXML
    private Label rankBadge;
    @FXML
    private Label avatarInitial;
    @FXML
    private StackPane avatarCircle;
    @FXML
    private StackPane leaderboardOverlay;

    // Injected from <fx:include> — the nested controller for the leaderboard
    // overlay
    @FXML
    private LeaderboardController leaderboardOverlayController;

    // ========================================================================
    // State
    // ========================================================================

    private Timeline timerTimeline;
    private int currentQuestionNumber = 0;
    private static final int TOTAL_QUESTIONS = 10;

    // ========================================================================
    // Lifecycle
    // ========================================================================

    @FXML
    public void initialize() {
        System.out.println("[Arena] Initializing ArenaController...");

        // Wire the nested leaderboard controller
        if (leaderboardOverlayController != null) {
            leaderboardOverlayController.setArenaController(this);
        }

        // Register this controller as the message handler for the stream
        GrpcClientService.getInstance().setMessageHandler(this::handleServerMessage);

        // Bootstrap the UI with session data
        Platform.runLater(() -> {
            String initial = SessionData.username.isEmpty() ? "?"
                    : SessionData.username.substring(0, 1).toUpperCase();
            headerUsername.setText(SessionData.username);
            avatarInitial.setText(initial);
            scoreLabel.setText("0 pts");
            rankBadge.setText("# —");

            // Initial state: waiting for first question
            questionMeta.setText("");
            questionText.setText("⏳  Waiting for the game to start…");
            setAnswerButtonsEnabled(false);

            if (leaderboardOverlay != null) {
                leaderboardOverlay.setVisible(false);
                leaderboardOverlay.setManaged(false);
            }
        });
    }

    /**
     * Called from LobbyController right after the scene transitions.
     * Displays the first question that arrived via the stream.
     */
    public void displayFirstQuestion(QuestionPayload question) {
        showNewQuestion(question);
    }

    // ========================================================================
    // Stream message handler (called from gRPC background thread)
    // ========================================================================

    private void handleServerMessage(ServerMessage message) {
        if (message.hasNewQuestion()) {
            Platform.runLater(() -> showNewQuestion(message.getNewQuestion()));
        } else if (message.hasLeaderboard()) {
            Platform.runLater(() -> applyLeaderboardUpdate(message.getLeaderboard()));
        }
    }

    // ========================================================================
    // UI Event Handlers (FXML callbacks — always on FX thread)
    // ========================================================================

    @FXML
    private void handleAnswerClickedA() {
        submitAnswer("A");
    }

    @FXML
    private void handleAnswerClickedB() {
        submitAnswer("B");
    }

    @FXML
    private void handleAnswerClickedC() {
        submitAnswer("C");
    }

    @FXML
    private void handleAnswerClickedD() {
        submitAnswer("D");
    }

    private void submitAnswer(String letter) {
        // Apply visual feedback
        Button selected, o1, o2, o3;
        switch (letter) {
            case "A" -> {
                selected = answerBtnA;
                o1 = answerBtnB;
                o2 = answerBtnC;
                o3 = answerBtnD;
            }
            case "B" -> {
                selected = answerBtnB;
                o1 = answerBtnA;
                o2 = answerBtnC;
                o3 = answerBtnD;
            }
            case "C" -> {
                selected = answerBtnC;
                o1 = answerBtnA;
                o2 = answerBtnB;
                o3 = answerBtnD;
            }
            case "D" -> {
                selected = answerBtnD;
                o1 = answerBtnA;
                o2 = answerBtnB;
                o3 = answerBtnC;
            }
            default -> {
                return;
            }
        }

        if (!selected.getStyleClass().contains("btn-answer-selected")) {
            selected.getStyleClass().add("btn-answer-selected");
        }
        o1.getStyleClass().add("btn-answer-dimmed");
        o2.getStyleClass().add("btn-answer-dimmed");
        o3.getStyleClass().add("btn-answer-dimmed");
        setAnswerButtonsEnabled(false);

        // Send over the stream (non-blocking)
        GrpcClientService.getInstance().sendAnswer(letter);
    }

    // ========================================================================
    // UI Updates (always called from FX thread via Platform.runLater)
    // ========================================================================

    private void showNewQuestion(QuestionPayload question) {
        GrpcClientService.getInstance().markQuestionStart();

        currentQuestionNumber++;
        questionMeta.setText("Question " + currentQuestionNumber + " of " + TOTAL_QUESTIONS);
        questionText.setText(question.getText());

        // Populate answer buttons
        List<String> options = question.getOptionsList();
        String[] prefixes = { "🔴  A. ", "🔵  B. ", "🟡  C. ", "🟢  D. " };
        Button[] btns = { answerBtnA, answerBtnB, answerBtnC, answerBtnD };
        for (int i = 0; i < 4; i++) {
            btns[i].setText(i < options.size() ? prefixes[i] + options.get(i) : "");
        }

        resetAnswerButtonStyles();
        setAnswerButtonsEnabled(true);

        // Hide leaderboard if still visible
        if (leaderboardOverlay != null) {
            leaderboardOverlay.setVisible(false);
            leaderboardOverlay.setManaged(false);
        }

        // Fade in animation
        FadeTransition fade = new FadeTransition(Duration.millis(300), questionText.getParent());
        fade.setFromValue(0.7);
        fade.setToValue(1.0);
        fade.play();

        // Start timer (duration from server)
        startTimer(question.getTimeLimitSec());
    }

    private void applyLeaderboardUpdate(LeaderboardUpdate update) {
        stopTimer();
        setAnswerButtonsEnabled(false);

        // Update header with server-authoritative data
        if (update.hasCurrentPlayer()) {
            int score = update.getCurrentPlayer().getScore();
            int rank = update.getCurrentPlayer().getRank();
            SessionData.currentScore = score;
            SessionData.currentRank = rank;
            scoreLabel.setText(String.format("%,d pts", score));
            rankBadge.setText("🥇 #" + rank);
        }

        // Show leaderboard with a slide-up animation
        if (leaderboardOverlayController != null) {
            leaderboardOverlayController.populate(update);
        }

        if (leaderboardOverlay != null) {
            leaderboardOverlay.setVisible(true);
            leaderboardOverlay.setManaged(true);

            // Slide up from bottom
            leaderboardOverlay.setTranslateY(700);
            TranslateTransition slideUp = new TranslateTransition(Duration.millis(400), leaderboardOverlay);
            slideUp.setToY(0);
            slideUp.setInterpolator(Interpolator.EASE_OUT);
            slideUp.play();
        }
    }

    // ========================================================================
    // Timer
    // ========================================================================

    private void startTimer(int timeLimitSec) {
        if (timerTimeline != null)
            timerTimeline.stop();

        timerBar.getStyleClass().removeAll("timer-warning", "timer-danger");
        timerBar.setProgress(1.0);

        timerTimeline = new Timeline();
        final int TICKS_PER_SEC = 5;
        final int totalTicks = timeLimitSec * TICKS_PER_SEC;

        for (int i = 0; i <= totalTicks; i++) {
            double progress = 1.0 - ((double) i / totalTicks);
            double ms = i * (1000.0 / TICKS_PER_SEC);
            timerTimeline.getKeyFrames().add(
                    new javafx.animation.KeyFrame(Duration.millis(ms),
                            new javafx.animation.KeyValue(timerBar.progressProperty(), progress)));
        }

        // Update colors based on remaining time
        timerTimeline.currentTimeProperty().addListener((obs, old, now) -> {
            double pct = timerBar.getProgress() * 100;
            if (pct <= 25) {
                timerBar.getStyleClass().remove("timer-warning");
                if (!timerBar.getStyleClass().contains("timer-danger"))
                    timerBar.getStyleClass().add("timer-danger");
            } else if (pct <= 50) {
                timerBar.getStyleClass().remove("timer-danger");
                if (!timerBar.getStyleClass().contains("timer-warning"))
                    timerBar.getStyleClass().add("timer-warning");
            } else {
                timerBar.getStyleClass().removeAll("timer-warning", "timer-danger");
            }
        });

        // Timer expired → disable buttons and wait for leaderboard from server
        timerTimeline.setOnFinished(e -> setAnswerButtonsEnabled(false));
        timerTimeline.play();
    }

    private void stopTimer() {
        if (timerTimeline != null) {
            timerTimeline.stop();
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    private void resetAnswerButtonStyles() {
        for (Button b : new Button[] { answerBtnA, answerBtnB, answerBtnC, answerBtnD }) {
            b.getStyleClass().removeAll("btn-answer-selected", "btn-answer-dimmed");
        }
    }

    private void setAnswerButtonsEnabled(boolean enabled) {
        answerBtnA.setDisable(!enabled);
        answerBtnB.setDisable(!enabled);
        answerBtnC.setDisable(!enabled);
        answerBtnD.setDisable(!enabled);
    }
}

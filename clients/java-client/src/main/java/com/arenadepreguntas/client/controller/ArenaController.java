package com.arenadepreguntas.client.controller;

import com.arenadepreguntas.client.GrpcClientService;
import com.arenadepreguntas.client.SessionData;
import com.arenadepreguntas.grpc.game.EmojiEvent;
import com.arenadepreguntas.grpc.game.LeaderboardUpdate;
import com.arenadepreguntas.grpc.game.QuestionPayload;
import com.arenadepreguntas.grpc.game.ServerMessage;

import javafx.animation.FadeTransition;
import javafx.animation.Interpolator;
import javafx.animation.KeyFrame;
import javafx.animation.KeyValue;
import javafx.animation.ParallelTransition;
import javafx.animation.Timeline;
import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.geometry.Pos;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.layout.StackPane;
import javafx.util.Duration;

import java.util.List;

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
    @FXML
    private StackPane emojiOverlay;

    @FXML
    private LeaderboardController leaderboardOverlayController;

    // ========================================================================
    // State
    // ========================================================================

    private Timeline timerTimeline;
    private int currentQuestionNumber = 0;
    private static final int TOTAL_QUESTIONS = 10;

    /** True once this player has submitted an answer for the current question. */
    private volatile boolean hasAnswered = false;

    // ========================================================================
    // Lifecycle
    // ========================================================================

    @FXML
    public void initialize() {
        if (leaderboardOverlayController != null) {
            leaderboardOverlayController.setArenaController(this);
        }

        GrpcClientService.getInstance().setMessageHandler(this::handleServerMessage);

        // Already on the FX thread (called from FXMLLoader.load inside Platform.runLater).
        // Running directly ensures displayFirstQuestion() can overwrite this initial state.
        String initial = SessionData.username.isEmpty() ? "?"
                : SessionData.username.substring(0, 1).toUpperCase();
        headerUsername.setText(SessionData.username);
        avatarInitial.setText(initial);
        scoreLabel.setText("0 pts");
        rankBadge.setText("# —");
        questionMeta.setText("");
        questionText.setText("⏳  Waiting for the game to start…");
        setAnswerButtonsEnabled(false);

        if (leaderboardOverlay != null) {
            leaderboardOverlay.setVisible(false);
            leaderboardOverlay.setManaged(false);
        }
        if (emojiOverlay != null) {
            emojiOverlay.setPickOnBounds(false);
        }
    }

    public void displayFirstQuestion(QuestionPayload question) {
        showNewQuestion(question);
    }

    // ========================================================================
    // Stream message dispatcher
    // ========================================================================

    private void handleServerMessage(ServerMessage message) {
        if (message.hasNewQuestion()) {
            Platform.runLater(() -> showNewQuestion(message.getNewQuestion()));
        } else if (message.hasLeaderboard()) {
            Platform.runLater(() -> applyLeaderboardUpdate(message.getLeaderboard()));
        } else if (message.hasEmoji()) {
            Platform.runLater(() -> showEmojiReaction(message.getEmoji()));
        }
    }

    // ========================================================================
    // Answer buttons
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
        if (hasAnswered)
            return; // guard against double-tap
        hasAnswered = true;

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

        if (!selected.getStyleClass().contains("btn-answer-selected"))
            selected.getStyleClass().add("btn-answer-selected");
        o1.getStyleClass().add("btn-answer-dimmed");
        o2.getStyleClass().add("btn-answer-dimmed");
        o3.getStyleClass().add("btn-answer-dimmed");
        setAnswerButtonsEnabled(false);

        GrpcClientService.getInstance().sendAnswer(letter);
    }

    // ========================================================================
    // UI Updates
    // ========================================================================

    private void showNewQuestion(QuestionPayload question) {
        hasAnswered = false;
        GrpcClientService.getInstance().markQuestionStart();

        currentQuestionNumber++;
        questionMeta.setText("Question " + currentQuestionNumber + " of " + TOTAL_QUESTIONS);
        questionText.setText(question.getText());

        List<String> options = question.getOptionsList();
        String[] prefixes = { "🔴  A. ", "🔵  B. ", "🟡  C. ", "🟢  D. " };
        Button[] btns = { answerBtnA, answerBtnB, answerBtnC, answerBtnD };
        for (int i = 0; i < 4; i++) {
            btns[i].setText(i < options.size() ? prefixes[i] + options.get(i) : "");
        }

        resetAnswerButtonStyles();
        setAnswerButtonsEnabled(true);

        if (leaderboardOverlay != null) {
            leaderboardOverlay.setVisible(false);
            leaderboardOverlay.setManaged(false);
        }

        FadeTransition fade = new FadeTransition(Duration.millis(300), questionText.getParent());
        fade.setFromValue(0.7);
        fade.setToValue(1.0);
        fade.play();

        startTimer(question.getTimeLimitSec());
    }

    private void applyLeaderboardUpdate(LeaderboardUpdate update) {
        // Only update this client's own score/rank and freeze UI when OUR answer is
        // acknowledged. Other players' answers come through here too — update the
        // top-players list but leave timer and buttons alone for those.
        boolean isOwnUpdate = update.hasCurrentPlayer()
                && (!update.getCurrentPlayer().getUserId().isEmpty()
                        ? update.getCurrentPlayer().getUserId().equals(SessionData.userId)
                        : update.getCurrentPlayer().getUsername().equals(SessionData.username));

        if (isOwnUpdate) {
            stopTimer();
            setAnswerButtonsEnabled(false);

            int score = update.getCurrentPlayer().getScore();
            int rank = update.getCurrentPlayer().getRank();
            SessionData.currentScore = score;
            SessionData.currentRank = rank;
            scoreLabel.setText(String.format("%,d pts", score));
            rankBadge.setText(rankEmoji(rank) + " #" + rank);
        }

        // Always refresh the leaderboard card so everyone sees live standings
        if (leaderboardOverlayController != null) {
            leaderboardOverlayController.populate(update);
        }

        // Only slide the overlay up once this player has answered
        if (isOwnUpdate && leaderboardOverlay != null) {
            leaderboardOverlay.setVisible(true);
            leaderboardOverlay.setManaged(true);
            leaderboardOverlay.setTranslateY(700);
            TranslateTransition slideUp = new TranslateTransition(Duration.millis(400), leaderboardOverlay);
            slideUp.setToY(0);
            slideUp.setInterpolator(Interpolator.EASE_OUT);
            slideUp.play();
        }
    }

    // ========================================================================
    // Emoji reactions
    // ========================================================================

    private void showEmojiReaction(EmojiEvent event) {
        if (emojiOverlay == null)
            return;

        Label bubble = new Label(event.getEmojiCode());
        bubble.setStyle("-fx-font-size: 48; -fx-effect: dropshadow(gaussian, rgba(0,0,0,0.5), 8, 0, 0, 2);");
        bubble.setAlignment(Pos.CENTER);

        // Random horizontal scatter so multiple emojis don't stack
        double xOffset = (Math.random() * 300) - 150;
        bubble.setTranslateX(xOffset);
        bubble.setTranslateY(60);
        bubble.setOpacity(1.0);

        emojiOverlay.getChildren().add(bubble);

        TranslateTransition rise = new TranslateTransition(Duration.millis(1800), bubble);
        rise.setByY(-220);
        rise.setInterpolator(Interpolator.EASE_OUT);

        FadeTransition fade = new FadeTransition(Duration.millis(1800), bubble);
        fade.setFromValue(1.0);
        fade.setToValue(0.0);

        ParallelTransition anim = new ParallelTransition(rise, fade);
        anim.setOnFinished(e -> emojiOverlay.getChildren().remove(bubble));
        anim.play();
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
                    new KeyFrame(Duration.millis(ms),
                            new KeyValue(timerBar.progressProperty(), progress)));
        }

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

        timerTimeline.setOnFinished(e -> setAnswerButtonsEnabled(false));
        timerTimeline.play();
    }

    private void stopTimer() {
        if (timerTimeline != null)
            timerTimeline.stop();
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

    private String rankEmoji(int rank) {
        return switch (rank) {
            case 1 -> "🥇";
            case 2 -> "🥈";
            case 3 -> "🥉";
            default -> "🏅";
        };
    }
}

package com.arenadepreguntas.client.controller;

import com.arenadepreguntas.client.SessionData;
import com.arenadepreguntas.grpc.game.LeaderboardUpdate;
import com.arenadepreguntas.grpc.game.PlayerScore;

import javafx.animation.TranslateTransition;
import javafx.event.ActionEvent;
import javafx.fxml.FXML;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.layout.HBox;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.VBox;
import javafx.util.Duration;

import java.util.List;

/**
 * Leaderboard controller manages the slide-up overlay showing top players and
 * current player rank/score.
 * Populated from server LeaderboardUpdate messages sent after each question is
 * answered.
 */
public class LeaderboardController {

    // ========================================================================
    // FXML bindings — top-4 rows + self row
    // ========================================================================

    @FXML
    private StackPane leaderboardOverlay;
    @FXML
    private VBox leaderboardCard;
    @FXML
    private Button nextQuestionButton;

    @FXML
    private HBox row1;
    @FXML
    private HBox row2;
    @FXML
    private HBox row3;
    @FXML
    private HBox row4;

    @FXML
    private Label rank1;
    @FXML
    private Label rank2;
    @FXML
    private Label rank3;
    @FXML
    private Label rank4;

    @FXML
    private Label username1;
    @FXML
    private Label username2;
    @FXML
    private Label username3;
    @FXML
    private Label username4;

    @FXML
    private Label score1;
    @FXML
    private Label score2;
    @FXML
    private Label score3;
    @FXML
    private Label score4;

    @FXML
    private HBox selfRow;
    @FXML
    private Label selfRank;
    @FXML
    private Label selfLeaderboardUsername;
    @FXML
    private Label selfLeaderboardScore;

    @FXML
    public void initialize() {
    }

    public void setArenaController(ArenaController controller) {
        // Reference kept for potential future use (e.g. end-of-game callbacks).
    }

    // ========================================================================
    // Accessor used by ArenaController to drive the slide-up animation
    // ========================================================================

    public VBox getLeaderboardCard() {
        return leaderboardCard;
    }

    // ========================================================================
    // Populate from a real LeaderboardUpdate (called on the FX thread)
    // ========================================================================

    public void populate(LeaderboardUpdate update) {
        List<PlayerScore> top = update.getTopPlayersList();

        HBox[] rowBoxes = { row1, row2, row3, row4 };
        Label[] rankLabels = { rank1, rank2, rank3, rank4 };
        Label[] userLabels = { username1, username2, username3, username4 };
        Label[] scoreLabels = { score1, score2, score3, score4 };

        for (int i = 0; i < 4; i++) {
            if (i < top.size()) {
                PlayerScore p = top.get(i);
                rankLabels[i].setText(rankText(i + 1));
                userLabels[i].setText(p.getUsername());
                scoreLabels[i].setText(String.format("%,d", p.getScore()));
                rowBoxes[i].setVisible(true);
                rowBoxes[i].setManaged(true);
            } else {
                rowBoxes[i].setVisible(false);
                rowBoxes[i].setManaged(false);
            }
        }

        // Self row is always shown at the bottom.
        PlayerScore self = update.getCurrentPlayer();
        selfRank.setText(rankText(self.getRank()));
        selfLeaderboardUsername.setText(SessionData.username);
        selfLeaderboardScore.setText(String.format("%,d", self.getScore()));
    }

    // ========================================================================
    // NEXT QUESTION button — dismisses the overlay; server drives the next round
    // ========================================================================

    @FXML
    private void handleNextQuestion(ActionEvent event) {
        if (leaderboardCard == null)
            return;

        TranslateTransition slideDown = new TranslateTransition(Duration.millis(200), leaderboardCard);
        slideDown.setToY(600);
        slideDown.setOnFinished(e -> {
            leaderboardOverlay.setVisible(false);
            leaderboardOverlay.setManaged(false);
            // Reset the card position so it can slide up again next round.
            leaderboardCard.setTranslateY(0);
        });
        slideDown.play();
    }

    // ========================================================================
    // Helper
    // ========================================================================

    private String rankText(int rank) {
        return switch (rank) {
            case 1 -> "🥇 1";
            case 2 -> "🥈 2";
            case 3 -> "🥉 3";
            default -> String.valueOf(rank);
        };
    }
}

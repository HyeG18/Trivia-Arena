import threading
from PyQt6.QtCore import QThread, pyqtSignal

from networking.grpc_client import ModeratorGrpcClient


class GrpcStreamWorker(QThread):
    """
    Background QThread that owns the PlayStream connection.

    Signals (always emitted to the main/GUI thread via Qt's queued connection):
      leaderboard_updated(players: list[dict], total_responses: int)
      players_connected_changed(count: int)
      question_launched(text: str, options: list[str], time_limit: int)
      emoji_received(username: str, emoji_code: str)
      error_occurred(message: str)
    """

    leaderboard_updated = pyqtSignal(list, int)
    players_connected_changed = pyqtSignal(int)
    roster_updated = pyqtSignal(list, list, int, bool, int)
    question_launched = pyqtSignal(str, list, int)
    emoji_received = pyqtSignal(str, str)
    error_occurred = pyqtSignal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._stop_event = threading.Event()
        self._client = ModeratorGrpcClient()

    # ------------------------------------------------------------------ #
    # QThread entry point
    # ------------------------------------------------------------------ #

    def run(self):
        try:
            responses = self._client.open_play_stream(self._stop_event)
            for msg in responses:
                if self._stop_event.is_set():
                    break

                if msg.HasField("leaderboard"):
                    lb = msg.leaderboard
                    players = [
                        {
                            "rank": p.rank,
                            "username": p.username,
                            "score": p.score,
                            "last_correct": p.last_answer_correct,
                        }
                        for p in lb.top_players
                    ]
                    # Qt queued connection ensures this runs on the GUI thread.
                    self.leaderboard_updated.emit(players, lb.total_responses)

                elif msg.HasField("new_question"):
                    q = msg.new_question
                    self.question_launched.emit(q.text, list(q.options), q.time_limit_sec)

                elif msg.HasField("emoji"):
                    e = msg.emoji
                    self.emoji_received.emit(e.username, e.emoji_code)
                elif msg.HasField("roster"):
                    roster = msg.roster
                    waiting = [
                        {"user_id": p.user_id, "username": p.username}
                        for p in roster.waiting
                    ]
                    approved = [
                        {"user_id": p.user_id, "username": p.username}
                        for p in roster.approved
                    ]
                    self.players_connected_changed.emit(roster.total_connected)
                    self.roster_updated.emit(
                        waiting,
                        approved,
                        roster.total_connected,
                        roster.game_started,
                        roster.total_responses,
                    )

        except Exception as exc:
            if not self._stop_event.is_set():
                self.error_occurred.emit(str(exc))

    # ------------------------------------------------------------------ #
    # Shutdown
    # ------------------------------------------------------------------ #

    def stop(self):
        self._stop_event.set()
        self._client.close()
        self.wait(3000)  # ms

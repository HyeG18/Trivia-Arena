import threading

from PyQt6.QtCore import QThread, pyqtSignal

from networking.grpc_client import ModeratorGrpcClient


class GrpcStreamWorker(QThread):
    """
    Background QThread that owns the PlayStream connection.

    Signals (always emitted to the main/GUI thread via Qt's queued connection):
      leaderboard_updated(players: list[dict], total_responses: int)
      error_occurred(message: str)
    """

    leaderboard_updated = pyqtSignal(list, int)
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

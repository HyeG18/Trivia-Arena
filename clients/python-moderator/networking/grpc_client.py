import threading
import grpc

from grpc_generated import game_pb2, game_pb2_grpc
import config


class ModeratorGrpcClient:
    """
    Wraps the GameService gRPC stub for moderator-specific operations.

    Moderator uses three RPCs from game.proto:
      - LaunchQuestion  (Unary)  — push a question to all players
      - ForceEndTimer   (Unary)  — close the current round early
      - PlayStream      (BidiStream, receive-only) — receive LeaderboardUpdates
    """

    def __init__(self):
        self._channel = grpc.insecure_channel(
            f"{config.GRPC_HOST}:{config.GRPC_PORT}"
        )
        self._stub = game_pb2_grpc.GameServiceStub(self._channel)

    # ------------------------------------------------------------------ #
    # Unary calls (safe to call from QThread workers)
    # ------------------------------------------------------------------ #

    def launch_question(
        self,
        text: str,
        options: list[str],
        time_limit_sec: int,
        correct_answer_index: int = 0,
    ) -> game_pb2.ModeratorAck:
        payload = game_pb2.QuestionPayload(
            text=text,
            options=options,
            time_limit_sec=time_limit_sec,
            correct_answer_index=correct_answer_index,
        )
        return self._stub.LaunchQuestion(payload)

    def force_end_timer(self) -> game_pb2.ModeratorAck:
        req = game_pb2.ForceEndRequest(moderator_id=config.MODERATOR_ID)
        return self._stub.ForceEndTimer(req)

    # ------------------------------------------------------------------ #
    # Bidirectional stream (moderator is receive-only)
    # ------------------------------------------------------------------ #

    def open_play_stream(self, stop_event: threading.Event):
        """
        Opens the PlayStream and returns the server-side response iterator.
        The request side is an empty generator that stays alive until stop_event
        is set — the moderator never sends ClientMessages over this stream.
        """

        def _empty_requests():
            # Yield nothing; block until shutdown is signalled.
            while not stop_event.is_set():
                stop_event.wait(timeout=0.5)

        return self._stub.PlayStream(_empty_requests())

    # ------------------------------------------------------------------ #
    # Lifecycle
    # ------------------------------------------------------------------ #

    def close(self) -> None:
        self._channel.close()

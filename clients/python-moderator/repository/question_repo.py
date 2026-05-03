from bson import ObjectId
from pymongo import MongoClient
import config


class QuestionRepository:
    """Thin wrapper around the MongoDB questions collection."""

    def __init__(self):
        self._client = MongoClient(config.MONGO_URI, serverSelectionTimeoutMS=5000)
        self._col = self._client[config.MONGO_DB][config.QUESTIONS_COLLECTION]

    # ------------------------------------------------------------------ #
    # Read
    # ------------------------------------------------------------------ #

    def get_all(self) -> list[dict]:
        return list(self._col.find())

    # ------------------------------------------------------------------ #
    # Write
    # ------------------------------------------------------------------ #

    def insert(
        self,
        text: str,
        options: list[str],
        correct_index: int,
        time_limit_sec: int = 20,
    ) -> str:
        doc = {
            "text": text,
            "options": options,
            "correct_index": correct_index,
            "time_limit_sec": time_limit_sec,
            "type": "multiple_choice",
        }
        result = self._col.insert_one(doc)
        return str(result.inserted_id)

    def delete(self, doc_id: str) -> None:
        self._col.delete_one({"_id": ObjectId(doc_id)})

    # ------------------------------------------------------------------ #
    # Lifecycle
    # ------------------------------------------------------------------ #

    def close(self) -> None:
        self._client.close()

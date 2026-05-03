from PyQt6.QtCore import QThread, pyqtSignal

from repository.question_repo import QuestionRepository


class LoadQuestionsWorker(QThread):
    finished = pyqtSignal(list)
    error = pyqtSignal(str)

    def run(self):
        try:
            repo = QuestionRepository()
            docs = repo.get_all()
            repo.close()
            self.finished.emit(docs)
        except Exception as exc:
            self.error.emit(str(exc))


class InsertQuestionWorker(QThread):
    finished = pyqtSignal(str)   # emits the new document's string id
    error = pyqtSignal(str)

    def __init__(self, text: str, options: list, correct_index: int, time_limit_sec: int, parent=None):
        super().__init__(parent)
        self._text = text
        self._options = options
        self._correct_index = correct_index
        self._time_limit_sec = time_limit_sec

    def run(self):
        try:
            repo = QuestionRepository()
            doc_id = repo.insert(self._text, self._options, self._correct_index, self._time_limit_sec)
            repo.close()
            self.finished.emit(doc_id)
        except Exception as exc:
            self.error.emit(str(exc))


class DeleteQuestionWorker(QThread):
    finished = pyqtSignal(str)   # emits the deleted document id
    error = pyqtSignal(str)

    def __init__(self, doc_id: str, parent=None):
        super().__init__(parent)
        self._doc_id = doc_id

    def run(self):
        try:
            repo = QuestionRepository()
            repo.delete(self._doc_id)
            repo.close()
            self.finished.emit(self._doc_id)
        except Exception as exc:
            self.error.emit(str(exc))

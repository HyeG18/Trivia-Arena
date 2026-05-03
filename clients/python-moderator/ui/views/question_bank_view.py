from PyQt6.QtWidgets import (
    QAbstractItemView,
    QButtonGroup,
    QFrame,
    QHBoxLayout,
    QHeaderView,
    QLabel,
    QLineEdit,
    QMessageBox,
    QPushButton,
    QRadioButton,
    QSpinBox,
    QTableWidget,
    QTableWidgetItem,
    QTextEdit,
    QVBoxLayout,
    QWidget,
)
from PyQt6.QtCore import Qt

from workers.db_worker import DeleteQuestionWorker, InsertQuestionWorker, LoadQuestionsWorker


class QuestionBankView(QWidget):
    """
    View 1 — Question Bank (MongoDB CRUD).

    Left panel : paginated table of stored questions.
    Right panel : form to add a new question directly into MongoDB.

    All database I/O runs on QThread workers — the GUI thread never blocks.
    """

    def __init__(self, parent=None):
        super().__init__(parent)
        self._questions: list[dict] = []
        self._workers: list = []        # keep worker refs alive until finished
        self._build_ui()
        self._load_questions()

    # ------------------------------------------------------------------ #
    # UI construction
    # ------------------------------------------------------------------ #

    def _build_ui(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(32, 24, 32, 24)
        root.setSpacing(20)

        # Header row
        header = QHBoxLayout()
        title = QLabel("Question Bank")
        title.setObjectName("viewTitle")
        header.addWidget(title)
        header.addStretch()
        btn_refresh = QPushButton("↻  Refresh")
        btn_refresh.setObjectName("btnSecondary")
        btn_refresh.clicked.connect(self._load_questions)
        header.addWidget(btn_refresh)
        root.addLayout(header)

        # Main split
        split = QHBoxLayout()
        split.setSpacing(20)
        split.addWidget(self._build_table_card(), stretch=3)
        split.addWidget(self._build_form_card(),  stretch=2)
        root.addLayout(split)

    def _build_table_card(self) -> QFrame:
        card = QFrame()
        card.setObjectName("card")
        layout = QVBoxLayout(card)
        layout.setContentsMargins(0, 0, 0, 0)
        layout.setSpacing(0)

        # Card header
        ch = QHBoxLayout()
        ch.setContentsMargins(16, 12, 16, 12)
        lbl = QLabel("STORED QUESTIONS")
        lbl.setObjectName("sectionTitle")
        ch.addWidget(lbl)
        ch.addStretch()
        self._count_label = QLabel("— questions")
        self._count_label.setObjectName("statusLabel")
        ch.addWidget(self._count_label)
        layout.addLayout(ch)

        divider = QFrame()
        divider.setObjectName("divider")
        divider.setFrameShape(QFrame.Shape.HLine)
        layout.addWidget(divider)

        # Table
        self._table = QTableWidget()
        self._table.setColumnCount(4)
        self._table.setHorizontalHeaderLabels(["ID", "Question", "Options", "Time (s)"])
        hh = self._table.horizontalHeader()
        hh.setSectionResizeMode(0, QHeaderView.ResizeMode.ResizeToContents)
        hh.setSectionResizeMode(1, QHeaderView.ResizeMode.Stretch)
        hh.setSectionResizeMode(2, QHeaderView.ResizeMode.ResizeToContents)
        hh.setSectionResizeMode(3, QHeaderView.ResizeMode.ResizeToContents)
        self._table.setSelectionBehavior(QAbstractItemView.SelectionBehavior.SelectRows)
        self._table.setEditTriggers(QAbstractItemView.EditTrigger.NoEditTriggers)
        self._table.setAlternatingRowColors(True)
        self._table.verticalHeader().setVisible(False)
        layout.addWidget(self._table)

        # Delete button
        btn_del = QPushButton("🗑  Delete Selected")
        btn_del.setObjectName("btnDanger")
        btn_del.clicked.connect(self._delete_selected)
        footer = QHBoxLayout()
        footer.setContentsMargins(0, 8, 12, 12)
        footer.addStretch()
        footer.addWidget(btn_del)
        layout.addLayout(footer)

        return card

    def _build_form_card(self) -> QFrame:
        card = QFrame()
        card.setObjectName("card")
        layout = QVBoxLayout(card)
        layout.setContentsMargins(20, 20, 20, 20)
        layout.setSpacing(14)

        form_title = QLabel("ADD QUESTION")
        form_title.setObjectName("sectionTitle")
        layout.addWidget(form_title)

        # Question text
        layout.addWidget(QLabel("Question Text"))
        self._q_text = QTextEdit()
        self._q_text.setPlaceholderText("Enter the question here…")
        self._q_text.setMaximumHeight(80)
        layout.addWidget(self._q_text)

        # Answer options (A–D) with radio buttons to mark the correct one
        layout.addWidget(QLabel("Answer Options  (● = correct)"))
        self._opt_inputs: list[QLineEdit] = []
        self._radio_group = QButtonGroup(self)
        for i, letter in enumerate("ABCD"):
            row = QHBoxLayout()
            radio = QRadioButton()
            radio.setToolTip("Mark as correct answer")
            self._radio_group.addButton(radio, i)
            row.addWidget(radio)
            inp = QLineEdit()
            inp.setPlaceholderText(f"Option {letter}")
            self._opt_inputs.append(inp)
            row.addWidget(inp)
            layout.addLayout(row)
        self._radio_group.button(0).setChecked(True)

        # Time limit
        time_row = QHBoxLayout()
        time_row.addWidget(QLabel("Time Limit (s)"))
        self._time_spin = QSpinBox()
        self._time_spin.setRange(5, 120)
        self._time_spin.setValue(20)
        self._time_spin.setFixedWidth(80)
        time_row.addWidget(self._time_spin)
        time_row.addStretch()
        layout.addLayout(time_row)

        layout.addStretch()

        btn_add = QPushButton("＋  Add Question")
        btn_add.setObjectName("btnSuccess")
        btn_add.clicked.connect(self._add_question)
        layout.addWidget(btn_add)

        return card

    # ------------------------------------------------------------------ #
    # Workers
    # ------------------------------------------------------------------ #

    def _load_questions(self) -> None:
        worker = LoadQuestionsWorker(self)
        worker.finished.connect(self._on_loaded)
        worker.error.connect(self._on_db_error)
        worker.finished.connect(worker.deleteLater)
        self._workers.append(worker)
        worker.start()

    def _on_loaded(self, docs: list) -> None:
        self._questions = docs
        n = len(docs)
        self._count_label.setText(f"{n} question{'s' if n != 1 else ''}")
        self._table.setRowCount(n)
        for row, doc in enumerate(docs):
            doc_id = str(doc["_id"])
            # Show only the last 8 hex chars to keep the column narrow
            id_item = QTableWidgetItem(f"…{doc_id[-8:]}")
            id_item.setData(Qt.ItemDataRole.UserRole, doc_id)
            self._table.setItem(row, 0, id_item)
            self._table.setItem(row, 1, QTableWidgetItem(doc.get("text", "")))
            opts = doc.get("options", [])
            self._table.setItem(row, 2, QTableWidgetItem(f"{len(opts)} opts"))
            self._table.setItem(row, 3, QTableWidgetItem(str(doc.get("time_limit_sec", 20))))

    def _add_question(self) -> None:
        text = self._q_text.toPlainText().strip()
        options = [inp.text().strip() for inp in self._opt_inputs]
        correct = self._radio_group.checkedId()
        time_limit = self._time_spin.value()

        if not text:
            QMessageBox.warning(self, "Validation", "Question text cannot be empty.")
            return
        valid_options = [o for o in options if o]
        if len(valid_options) < 2:
            QMessageBox.warning(self, "Validation", "Provide at least 2 answer options.")
            return

        worker = InsertQuestionWorker(text, valid_options, correct, time_limit, self)
        worker.finished.connect(lambda _: self._load_questions())
        worker.error.connect(self._on_db_error)
        worker.finished.connect(worker.deleteLater)
        self._workers.append(worker)
        worker.start()

        # Clear the form
        self._q_text.clear()
        for inp in self._opt_inputs:
            inp.clear()
        self._radio_group.button(0).setChecked(True)

    def _delete_selected(self) -> None:
        row = self._table.currentRow()
        if row < 0:
            return
        item = self._table.item(row, 0)
        if not item:
            return
        doc_id = item.data(Qt.ItemDataRole.UserRole)

        worker = DeleteQuestionWorker(doc_id, self)
        worker.finished.connect(lambda _: self._load_questions())
        worker.error.connect(self._on_db_error)
        worker.finished.connect(worker.deleteLater)
        self._workers.append(worker)
        worker.start()

    def _on_db_error(self, msg: str) -> None:
        QMessageBox.critical(self, "Database Error", msg)

    # ------------------------------------------------------------------ #
    # Public API (used by LiveRoomView)
    # ------------------------------------------------------------------ #

    def get_questions(self) -> list[dict]:
        """Returns the in-memory question cache for the Live Room combo-box."""
        return self._questions

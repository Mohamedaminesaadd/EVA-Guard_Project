
from pathlib import Path
from typing import List
import re

from bs4 import BeautifulSoup
from langchain_core.documents import Document
from langchain_community.document_loaders import PyPDFLoader


class DocumentLoader:

    SUPPORTED_EXTENSIONS = {
        ".pdf",
        ".md",
        ".txt",
        ".html",
        ".htm",
        ".yaml",
        ".yml"
    }

    def __init__(self, documents_dir: str = None):

        project_root = Path(__file__).resolve().parents[2]

        self.documents_dir = (
            Path(documents_dir)
            if documents_dir
            else project_root / "knowledge_base" / "documents"
        )

        self.documents_dir = self.documents_dir.resolve()

        if not self.documents_dir.exists():
            raise FileNotFoundError(
                f"Documents directory not found: {self.documents_dir}"
            )

    def _clean_text(self, text: str) -> str:

        text = text.replace("\x00", "")
        text = re.sub(r"[ \t]+", " ", text)
        text = re.sub(r"\n{3,}", "\n\n", text)

        return text.strip()

    def _metadata(self, file_path: Path) -> dict:

        try:
            relative_path = file_path.relative_to(
                self.documents_dir
            )

            parts = relative_path.parts

            category = parts[0] if len(parts) > 0 else "unknown"
            subcategory = parts[1] if len(parts) > 2 else "general"

        except (ValueError, IndexError):
            category = "unknown"
            subcategory = "general"

        return {
            "source": str(file_path.resolve()),
            "file_name": file_path.name,
            "category": category,
            "subcategory": subcategory,
            "file_type": file_path.suffix.lower(),
            "document_type": "technical_document"
        }

    def _load_pdf(self, file_path: Path) -> List[Document]:

        loader = PyPDFLoader(str(file_path))
        pages = loader.load()

        documents = []

        for page in pages:

            text = self._clean_text(page.page_content)

            if not text:
                continue

            metadata = self._metadata(file_path)
            metadata.update(page.metadata)

            metadata["page_number"] = (
                int(page.metadata.get("page", 0)) + 1
            )

            documents.append(
                Document(
                    page_content=text,
                    metadata=metadata
                )
            )

        return documents

    def _load_text(self, file_path: Path) -> List[Document]:

        text = file_path.read_text(
            encoding="utf-8",
            errors="replace"
        )

        text = self._clean_text(text)

        if not text:
            return []

        metadata = self._metadata(file_path)

        return [
            Document(
                page_content=text,
                metadata=metadata
            )
        ]

    def _load_yaml(self, file_path: Path) -> List[Document]:

        text = file_path.read_text(
            encoding="utf-8",
            errors="replace"
        )

        text = self._clean_text(text)

        if not text:
            return []

        metadata = self._metadata(file_path)
        metadata["document_type"] = "api_specification"
        metadata["title"] = file_path.stem

        return [
            Document(
                page_content=text,
                metadata=metadata
            )
        ]

    def _load_html(self, file_path: Path) -> List[Document]:

        html = file_path.read_text(
            encoding="utf-8",
            errors="replace"
        )

        soup = BeautifulSoup(html, "html.parser")

        for element in soup(
            ["script", "style", "noscript", "svg"]
        ):
            element.decompose()

        title = (
            soup.title.get_text(strip=True)
            if soup.title
            else file_path.stem
        )

        text = soup.get_text(separator="\n")
        text = self._clean_text(text)

        if not text:
            return []

        metadata = self._metadata(file_path)
        metadata["title"] = title

        return [
            Document(
                page_content=text,
                metadata=metadata
            )
        ]

    def load_file(self, file_path: Path) -> List[Document]:

        extension = file_path.suffix.lower()

        if extension not in self.SUPPORTED_EXTENSIONS:
            return []

        try:

            if extension == ".pdf":
                return self._load_pdf(file_path)

            if extension in {".md", ".txt"}:
                return self._load_text(file_path)

            if extension in {".yaml", ".yml"}:
                return self._load_yaml(file_path)

            if extension in {".html", ".htm"}:
                return self._load_html(file_path)

        except Exception as error:

            print(
                f"[ERROR] Cannot load {file_path}: {error}"
            )

        return []

    def load_all(self) -> List[Document]:

        documents = []

        files = sorted(
            self.documents_dir.rglob("*")
        )

        supported_files = [
            file_path
            for file_path in files
            if file_path.is_file()
            and file_path.suffix.lower() in self.SUPPORTED_EXTENSIONS
        ]

        total_files = len(supported_files)

        print(f"\nFound {total_files} supported files.\n")

        for index, file_path in enumerate(supported_files, start=1):

            loaded = self.load_file(file_path)

            documents.extend(loaded)

            print(
                f"[{index}/{total_files}] "
                f"[LOADED] {file_path.name}: "
                f"{len(loaded)} document(s)"
            )

        print("\n========== LOADING SUMMARY ==========")
        print(f"Files found: {total_files}")
        print(f"Documents loaded: {len(documents)}")

        return documents


if __name__ == "__main__":

    loader = DocumentLoader()

    documents = loader.load_all()

    print("\n========== FIRST DOCUMENT PREVIEW ==========")

    if documents:

        print(documents[0].page_content[:500])
        print(documents[0].metadata)

    else:

        print("No documents were loaded.")

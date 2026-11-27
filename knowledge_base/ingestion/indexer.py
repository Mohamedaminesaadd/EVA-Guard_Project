
import argparse
import hashlib
from pathlib import Path
from typing import List

import chromadb

from langchain_core.documents import Document
from langchain_core.embeddings import Embeddings
from langchain_ollama import OllamaEmbeddings
from langchain_chroma import Chroma

from .document_loader import DocumentLoader
from .chunker import DocumentChunker


# ==============================
# CONFIGURATION
# ==============================

PROJECT_ROOT = Path(__file__).resolve().parents[2]

CHROMA_HOST = "localhost"
CHROMA_PORT = 8000

OLLAMA_URL = "http://localhost:11434"

COLLECTION_NAME = "eva_guard_knowledge"

EMBEDDING_MODEL = "nomic-embed-text:latest"

CHUNK_SIZE = 1000
CHUNK_OVERLAP = 150

BATCH_SIZE = 64


# ==============================
# NOMIC EMBEDDINGS
# ==============================

class NomicEmbeddings(Embeddings):

    def __init__(
        self,
        model: str = EMBEDDING_MODEL,
        base_url: str = OLLAMA_URL
    ):

        self.model = model

        self.embedding_model = OllamaEmbeddings(
            model=model,
            base_url=base_url
        )

    def embed_documents(
        self,
        texts: List[str]
    ) -> List[List[float]]:

        prefixed_texts = [
            f"search_document: {text}"
            for text in texts
        ]

        return self.embedding_model.embed_documents(
            prefixed_texts
        )

    def embed_query(
        self,
        text: str
    ) -> List[float]:

        prefixed_text = f"search_query: {text}"

        return self.embedding_model.embed_query(
            prefixed_text
        )


# ==============================
# CHROMA INDEXER
# ==============================

class KnowledgeIndexer:

    def __init__(
        self,
        collection_name: str = COLLECTION_NAME,
        host: str = CHROMA_HOST,
        port: int = CHROMA_PORT
    ):

        self.collection_name = collection_name

        print("[INFO] Connecting to ChromaDB...")

        self.client = chromadb.HttpClient(
            host=host,
            port=port
        )

        self.client.heartbeat()

        print("[OK] ChromaDB connection established.")

        print("[INFO] Initializing Nomic embeddings...")

        self.embeddings = NomicEmbeddings()

        # Verify that Ollama and the embedding model work.
        test_vector = self.embeddings.embed_query(
            "Ethereum node security"
        )

        print(
            f"[OK] Nomic embedding initialized. "
            f"Dimension: {len(test_vector)}"
        )

        self._initialize_vector_store()

    def _initialize_vector_store(self):

        self.vector_store = Chroma(
            client=self.client,
            collection_name=self.collection_name,
            embedding_function=self.embeddings,
            collection_metadata={
                "hnsw:space": "cosine"
            }
        )

    # ==============================
    # DOCUMENT IDS
    # ==============================

    def _generate_id(self, document: Document) -> str:

        source = str(
            document.metadata.get("source", "")
        )

        page = str(
            document.metadata.get("page_number", "")
        )

        chunk_index = str(
            document.metadata.get("chunk_index", "")
        )

        content = document.page_content

        unique_content = (
            source
            + "|"
            + page
            + "|"
            + chunk_index
            + "|"
            + content
        )

        return hashlib.sha256(
            unique_content.encode("utf-8")
        ).hexdigest()

    # ==============================
    # METADATA CLEANING
    # ==============================

    def _clean_metadata(self, metadata: dict) -> dict:

        clean = {}

        for key, value in metadata.items():

            if value is None:
                continue

            if isinstance(value, (str, int, float, bool)):
                clean[key] = value

            elif isinstance(value, (list, tuple)):
                clean[key] = ", ".join(
                    str(item) for item in value
                )

            else:
                clean[key] = str(value)

        return clean

    # ==============================
    # INDEX DOCUMENTS
    # ==============================

    def index_documents(
        self,
        chunks: List[Document]
    ):

        if not chunks:

            print("[WARNING] No chunks to index.")
            return

        print("\n========== CHROMA INDEXING ==========")

        total = len(chunks)

        for start in range(0, total, BATCH_SIZE):

            batch = chunks[
                start:start + BATCH_SIZE
            ]

            ids = [
                self._generate_id(doc)
                for doc in batch
            ]

            cleaned_documents = []

            for doc in batch:

                doc.metadata = self._clean_metadata(
                    doc.metadata
                )

                cleaned_documents.append(doc)

            self.vector_store.add_documents(
                documents=cleaned_documents,
                ids=ids
            )

            processed = min(
                start + len(batch),
                total
            )

            print(
                f"[INDEXED] {processed}/{total} chunks"
            )

        print("\n========== INDEXING COMPLETED ==========")

        print("Collection:", self.collection_name)

        print(
            "Total vectors:",
            self.vector_store._collection.count()
        )

    # ==============================
    # SEMANTIC SEARCH
    # ==============================

    def search(
        self,
        query: str,
        k: int = 5,
        category: str = None
    ):

        if k <= 0:
            raise ValueError("k must be positive")

        filter_metadata = None

        if category:

            filter_metadata = {
                "category": category
            }

        results = self.vector_store.similarity_search_with_score(
            query=query,
            k=k,
            filter=filter_metadata
        )

        print("\n========== SEARCH RESULTS ==========")

        if not results:
            print("No relevant documents found.")
            return results

        for index, (document, distance) in enumerate(
            results,
            start=1
        ):

            print(f"\nResult {index}")

            print(f"Distance: {distance:.4f}")

            print(
                "Category:",
                document.metadata.get("category")
            )

            print(
                "File:",
                document.metadata.get("file_name")
            )

            print(
                "Source:",
                document.metadata.get("source")
            )

            print(
                "Chunk:",
                document.metadata.get("chunk_id")
            )

            print(
                "Content:",
                document.page_content[:500]
            )

            print("-" * 60)

        return results

    # ==============================
    # COLLECTION MANAGEMENT
    # ==============================

    def reset_collection(self):

        print(
            f"\n[WARNING] Deleting collection: "
            f"{self.collection_name}"
        )

        try:

            self.client.delete_collection(
                self.collection_name
            )

            print("[OK] Old collection deleted.")

        except Exception as error:

            print("[INFO] Collection deletion:", error)

        self._initialize_vector_store()

        print("[OK] New collection created.")

    def collection_count(self) -> int:

        return self.vector_store._collection.count()


# ==============================
# MAIN PIPELINE
# ==============================

def main():

    parser = argparse.ArgumentParser(
        description="EVA-Guard RAG Knowledge Indexer"
    )

    group = parser.add_mutually_exclusive_group()

    group.add_argument(
        "--rebuild",
        action="store_true",
        help="Delete and rebuild the Chroma collection"
    )

    group.add_argument(
        "--search",
        type=str,
        help="Search the indexed knowledge base"
    )

    group.add_argument(
        "--count",
        action="store_true",
        help="Display the number of indexed vectors"
    )

    parser.add_argument(
        "--category",
        type=str,
        help="Filter search by category"
    )

    parser.add_argument(
        "--k",
        type=int,
        default=5
    )

    args = parser.parse_args()

    indexer = KnowledgeIndexer()

    # ==============================
    # COUNT
    # ==============================

    if args.count:

        print(
            "\nTotal vectors:",
            indexer.collection_count()
        )

        return

    # ==============================
    # SEARCH
    # ==============================

    if args.search:

        indexer.search(
            query=args.search,
            k=args.k,
            category=args.category
        )

        return

    # ==============================
    # REBUILD
    # ==============================

    if args.rebuild:

        indexer.reset_collection()

    # ==============================
    # 1. LOAD DOCUMENTS
    # ==============================

    print("\n========== STEP 1: DOCUMENT LOADING ==========")

    loader = DocumentLoader()

    documents = loader.load_all()

    if not documents:

        print("[ERROR] No documents found.")
        return

    # ==============================
    # 2. CHUNK DOCUMENTS
    # ==============================

    print("\n========== STEP 2: DOCUMENT CHUNKING ==========")

    chunker = DocumentChunker(
        chunk_size=CHUNK_SIZE,
        chunk_overlap=CHUNK_OVERLAP
    )

    chunks = chunker.process(documents)

    if not chunks:

        print("[ERROR] No chunks generated.")
        return

    # ==============================
    # 3. INDEX DOCUMENTS
    # ==============================

    print("\n========== STEP 3: VECTOR INDEXING ==========")

    indexer.index_documents(chunks)

    print("\n========== PIPELINE COMPLETED ==========")

    print(
        "Final collection size:",
        indexer.collection_count()
    )


if __name__ == "__main__":

    main()

import chromadb

from typing import List, Optional, Dict, Any

from langchain_core.documents import Document
from langchain_chroma import Chroma

from knowledge_base.ingestion.indexer import (
    NomicEmbeddings,
    CHROMA_HOST,
    CHROMA_PORT,
    COLLECTION_NAME
)


class KnowledgeRetriever:

    def __init__(
        self,
        collection_name: str = COLLECTION_NAME,
        host: str = CHROMA_HOST,
        port: int = CHROMA_PORT,
        embedding_model: str = "nomic-embed-text:latest"
    ):

        print("[INFO] Initializing Knowledge Retriever...")

        self.collection_name = collection_name

        # 1. Initialize Nomic Embeddings

        self.embeddings = NomicEmbeddings(
            model=embedding_model
        )

        # 2. Connect to ChromaDB

        self.client = chromadb.HttpClient(
            host=host,
            port=port
        )

        self.client.heartbeat()

        # 3. Connect to existing vector collection

        self.vector_store = Chroma(
            client=self.client,
            collection_name=self.collection_name,
            embedding_function=self.embeddings
        )

        print("[OK] ChromaDB connected.")
        print("[OK] Knowledge Retriever initialized.")

        print(
            "[INFO] Collection vectors:",
            self.vector_store._collection.count()
        )

    # =====================================
    # SEMANTIC RETRIEVAL
    # =====================================

    def retrieve(
        self,
        query: str,
        k: int = 5,
        category: Optional[str] = None
    ) -> List[Dict[str, Any]]:

        if not query or not query.strip():
            raise ValueError("Query cannot be empty.")

        if k <= 0:
            raise ValueError("k must be positive.")

        # Metadata filtering

        search_filter = None

        if category:

            search_filter = {
                "category": category
            }

        # Search in ChromaDB

        results = self.vector_store.similarity_search_with_score(
            query=query,
            k=k,
            filter=search_filter
        )

        retrieved_documents = []

        for document, distance in results:

            retrieved_documents.append({

                "content": document.page_content,

                "metadata": document.metadata,

                "distance": float(distance)

            })

        return retrieved_documents

    # =====================================
    # RETRIEVE LANGCHAIN DOCUMENTS
    # =====================================

    def retrieve_documents(
        self,
        query: str,
        k: int = 5,
        category: Optional[str] = None
    ) -> List[Document]:

        if not query or not query.strip():
            raise ValueError("Query cannot be empty.")

        if k <= 0:
            raise ValueError("k must be positive.")

        search_filter = None

        if category:

            search_filter = {
                "category": category
            }

        return self.vector_store.similarity_search(
            query=query,
            k=k,
            filter=search_filter
        )

    # =====================================
    # DISPLAY RESULTS
    # =====================================

    def display_results(
        self,
        results: List[Dict[str, Any]]
    ):

        print("\n========== RETRIEVAL RESULTS ==========")

        if not results:

            print("No relevant documents found.")

            return

        for index, result in enumerate(
            results,
            start=1
        ):

            metadata = result["metadata"]

            print(f"\nResult {index}")

            print(
                f"Distance: {result['distance']:.4f}"
            )

            print(
                "Category:",
                metadata.get("category")
            )

            print(
                "File:",
                metadata.get("file_name")
            )

            print(
                "Source:",
                metadata.get("source")
            )

            print(
                "Chunk ID:",
                metadata.get("chunk_id")
            )

            print("\nContent:")

            print(
                result["content"][:500]
            )

            print("-" * 60)


# =====================================
# TEST
# =====================================

if __name__ == "__main__":

    retriever = KnowledgeRetriever()

    query = (
        "What are the indicators of an Ethereum eclipse attack?"
    )

    results = retriever.retrieve(
        query=query,
        k=5,
        category="attacks"
    )

    retriever.display_results(results)
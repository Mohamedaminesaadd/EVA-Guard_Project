
from typing import List

from langchain_core.documents import Document
from langchain_text_splitters import RecursiveCharacterTextSplitter


class DocumentChunker:

    def __init__(
        self,
        chunk_size: int = 1000,
        chunk_overlap: int = 150
    ):

        if chunk_size <= 0:
            raise ValueError("chunk_size must be positive")

        if chunk_overlap < 0 or chunk_overlap >= chunk_size:
            raise ValueError(
                "chunk_overlap must be >= 0 and < chunk_size"
            )

        self.chunk_size = chunk_size
        self.chunk_overlap = chunk_overlap

        self.splitter = RecursiveCharacterTextSplitter(
            chunk_size=self.chunk_size,
            chunk_overlap=self.chunk_overlap,
            separators=[
                "\n## ",
                "\n### ",
                "\n\n",
                "\n",
                ". ",
                " ",
                ""
            ],
            length_function=len,
            keep_separator=True
        )

    def split_documents(
        self,
        documents: List[Document]
    ) -> List[Document]:

        chunks = []

        for document_index, document in enumerate(documents):

            document_chunks = self.splitter.split_documents(
                [document]
            )

            for chunk_index, chunk in enumerate(document_chunks):

                chunk.metadata["document_index"] = document_index
                chunk.metadata["chunk_index"] = chunk_index

                chunk.metadata["chunk_id"] = (
                    f"doc_{document_index}_chunk_{chunk_index}"
                )

                chunk.metadata["chunk_size"] = len(
                    chunk.page_content
                )

                chunks.append(chunk)

        return chunks

    def process(
        self,
        documents: List[Document]
    ) -> List[Document]:

        chunks = self.split_documents(documents)

        print("\n========== CHUNKING SUMMARY ==========")
        print(f"Input documents: {len(documents)}")
        print(f"Generated chunks: {len(chunks)}")
        print(f"Chunk size: {self.chunk_size}")
        print(f"Chunk overlap: {self.chunk_overlap}")

        if chunks:

            average_size = sum(
                len(chunk.page_content)
                for chunk in chunks
            ) / len(chunks)

            print(f"Average chunk size: {average_size:.2f}")

            print(
                f"Minimum chunk size: "
                f"{min(len(c.page_content) for c in chunks)}"
            )

            print(
                f"Maximum chunk size: "
                f"{max(len(c.page_content) for c in chunks)}"
            )

        return chunks


if __name__ == "__main__":

    from .document_loader import DocumentLoader

    loader = DocumentLoader()

    documents = loader.load_all()

    chunker = DocumentChunker(
        chunk_size=1000,
        chunk_overlap=150
    )

    chunks = chunker.process(documents)

    if chunks:

        print("\n========== FIRST CHUNK PREVIEW ==========")

        print(chunks[0].page_content[:500])
        print(chunks[0].metadata)

    else:

        print("No chunks generated.")

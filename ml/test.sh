
#!/bin/bash

# ==========================================
# EVA-Guard - ML Pipeline Structure
# ==========================================

echo "Creating EVA-Guard ML project structure..."

# Data directories
mkdir -p data/{raw,interim,processed,splits}

# Notebooks
mkdir -p notebooks

# Source code directories
mkdir -p src/{ingestion,preprocessing,features,labeling,training,evaluation,inference}

# Models and artifacts
mkdir -p models
mkdir -p artifacts
mkdir -p configs

# Tests
mkdir -p tests

echo "Project structure created successfully!"

# Display structure
tree -d -L 3

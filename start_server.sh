#!/bin/bash
# Start the RAG HTTP Server

echo "============================================================"
echo "Blender Helper AI - Starting RAG Server"
echo "============================================================"
echo ""

cd rag_system

echo "Checking dependencies..."
python3 -c "import flask, sentence_transformers, numpy" 2>/dev/null

if [ $? -ne 0 ]; then
    echo ""
    echo "Missing dependencies! Installing..."
    python3 -m pip install -r requirements_server.txt
    echo ""
fi

echo "Starting server on http://127.0.0.1:5000"
echo ""
echo "Press Ctrl+C to stop"
echo ""

python3 server.py

@echo off
REM Start the RAG HTTP Server

echo ============================================================
echo Blender Helper AI - Starting RAG Server
echo ============================================================
echo.

cd rag_system

echo Checking dependencies...
python -c "import flask, sentence_transformers, numpy" 2>nul

if %errorlevel% neq 0 (
    echo.
    echo Missing dependencies! Installing...
    python -m pip install -r requirements_server.txt
    echo.
)

echo Starting server on http://127.0.0.1:5000
echo.
echo Press Ctrl+C to stop
echo.

python server.py
pause

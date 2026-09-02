# Expose the assessment CLI through the existing PyO3 extension

Camau remains a Python-first package but adds a supported `camau assess` command implemented with Rust and `clap`. A PEP 621 console entry point calls the Rust extension function so `pip install camau` installs the command without shipping a second copy of the native code; the existing Python assessor remains available for compatibility.

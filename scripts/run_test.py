#!/usr/bin/env python3
"""Simple test runner for ProtoDuck extension SQL tests."""

import sys
import duckdb

def run_test(test_file: str, extension_path: str) -> bool:
    """Run a SQL test file against DuckDB with the extension loaded."""
    try:
        # Connect to DuckDB with unsigned extension loading enabled
        conn = duckdb.connect(config={'allow_unsigned_extensions': True})

        # Load the extension
        conn.execute(f"LOAD '{extension_path}'")

        # Read and execute the test file
        with open(test_file, 'r') as f:
            sql_content = f.read()

        # Split into statements and execute
        statements = [s.strip() for s in sql_content.split(';') if s.strip()]

        for stmt in statements:
            if stmt.startswith('--'):
                continue
            try:
                result = conn.execute(stmt)
                # Print results for SELECT statements
                if stmt.strip().upper().startswith('SELECT'):
                    rows = result.fetchall()
                    print(f"Result: {rows}")
            except Exception as e:
                print(f"Error executing: {stmt}")
                print(f"  {e}")
                return False

        print(f"PASSED: {test_file}")
        return True

    except Exception as e:
        print(f"FAILED: {test_file}")
        print(f"  Error: {e}")
        return False

if __name__ == '__main__':
    if len(sys.argv) != 3:
        print("Usage: run_test.py <test_file> <extension_path>")
        sys.exit(1)

    success = run_test(sys.argv[1], sys.argv[2])
    sys.exit(0 if success else 1)

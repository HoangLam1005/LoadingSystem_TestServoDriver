# Hướng Dẫn Chạy Test

## 1. Rust Backend Unit Tests (không cần phần cứng)

```bash
cd rust_backend
cargo test -- --nocapture
```

## 2. Python HMI Unit Tests (không cần backend)

```bash
cd python_hmi
python -m pytest tests/ -v
```

## 3. Integration Tests (cần Rust backend đang chạy)

```bash
# Terminal 1: Khởi chạy backend
cd rust_backend
cargo run

# Terminal 2: Chạy integration tests
cd ..
python -m pytest tests/test_integration.py -v
```

> **Lưu ý:** Integration tests sẽ tự động bỏ qua nếu backend chưa chạy.

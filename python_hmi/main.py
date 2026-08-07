#!/usr/bin/env python3
# =============================================================================
# LoadingSystem HMI - Entry Point
# =============================================================================
# Khởi chạy giao diện HMI đĩa xoay.
#
# Cách dùng:
#   python main.py                          # Kết nối localhost:5555
#   python main.py --host 192.168.1.10      # Kết nối IP khác
#   python main.py --port 6666              # Kết nối port khác
# =============================================================================

import argparse
import sys


def main():
    parser = argparse.ArgumentParser(
        description="LoadingSystem HMI - Rotating Disc Controller Interface"
    )
    parser.add_argument(
        "--host",
        type=str,
        default="127.0.0.1",
        help="Backend daemon IP address (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=5555,
        help="Backend daemon TCP port (default: 5555)",
    )
    args = parser.parse_args()

    print(f"[HMI] Starting LoadingSystem HMI...")
    print(f"[HMI] Backend: {args.host}:{args.port}")

    try:
        from app.hmi_app import LoadingSystemHMI

        app = LoadingSystemHMI(
            backend_host=args.host,
            backend_port=args.port,
        )
        app.mainloop()
    except ImportError as e:
        print(f"\n[ERROR] Missing dependency: {e}")
        print("[ERROR] Please install: pip install -r requirements.txt")
        sys.exit(1)
    except Exception as e:
        print(f"\n[ERROR] Application error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()

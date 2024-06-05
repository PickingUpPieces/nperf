#!/bin/bash

python3 benchmark.py configs/send_methods_vs_uring_with_threads_client.json -m || true

python3 benchmark.py configs/send_methods_vs_uring_with_threads_client_gsro.json -m || true

python3 benchmark.py configs/send_methods_vs_uring_both.json -m || true

python3 benchmark.py configs/uring_task_work.json -m || true


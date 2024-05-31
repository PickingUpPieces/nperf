#!/bin/bash

# Function to handle SIGINT signal
cleanup() {
    echo "Received SIGINT signal. Terminating the process."
    kill $pid
    exit 1
}
trap cleanup SIGINT

NPERF_BIN="../target/release/nperf"
# Start the process with command line arguments

$NPERF_BIN "$@" &
pid=$!


# Get the PID of the last background process started
pid=$!

if [ -z "$pid" ]; then
    echo "Failed to start the process. Exiting."
    exit 1
fi

max_thread_count=0

# Loop to monitor the number of threads
while true; do
    # Check if the process has terminated
    if ! ps -p $pid > /dev/null; then
        break
    fi
    
    thread_count=$(ps -T -p $pid | wc -l)
    thread_count=$((thread_count - 1))  # Subtract 1 to exclude header
    #echo "Number of threads: $thread_count"
    
    # Update max_thread_count if the current count is higher
    if [ $thread_count -gt $max_thread_count ]; then
        max_thread_count=$thread_count
    fi

    #sleep 0.2  # Adjust the interval as needed
done

echo "Process has terminated."
echo "Highest number of threads counted during execution: $max_thread_count"

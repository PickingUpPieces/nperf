import pandas as pd
import matplotlib.pyplot as plt
import sys
import os

# Data should be in format from `pcm-memory`: https://github.com/intel/pcm with `sudo ./pcm-memory 0.1 -silent -nc -csv=test.log`

def plot_memory_bandwidth(filename):
    # Read the CSV file into a pandas DataFrame, skipping the first row
    try:
        df = pd.read_csv(filename, skiprows=1)
    except FileNotFoundError:
        print("Error: File not found.")
        return
    except pd.errors.EmptyDataError:
        print("Error: File is empty.")
        return

    # Extract relevant columns
    df = df.iloc[:, [0, 1, -3, -2, -1]]  # Keep Date, Time, and last three columns

    # Combine Date and Time columns into a single datetime column
    df['Datetime'] = pd.to_datetime(df['Date'] + ' ' + df['Time'])

    # Plot the data
    plt.figure(figsize=(10, 6))
    plt.plot(df['Datetime'], df['Read'], label="Memory Read (MB/s)")
    plt.plot(df['Datetime'], df['Write'], label="Memory Write (MB/s)")
    plt.xlabel("Time")
    plt.ylabel("Throughput (MB/s)")
    plt.title("System Memory Read and Write Throughput Over Time")
    plt.legend()
    plt.grid(True)
    plt.xticks(rotation=45)
    plt.tight_layout()

    # Save the plot
    output_filename = os.path.splitext(filename)[0] + '_graph.png'
    plt.savefig(output_filename)
    plt.show()

    print(f"Graph saved as: {output_filename}")

if __name__ == "__main__":
    if len(sys.argv) != 2:
        print("Usage: python script.py <filename.csv>")
        sys.exit(1)

    filename = sys.argv[1]
    plot_memory_bandwidth(filename)
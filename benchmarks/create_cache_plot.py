import pandas as pd
import matplotlib.pyplot as plt
import sys
import os

def plot_cache_hits(filename, plot_type):
    try:
        df = pd.read_csv(filename, skiprows=1)
    except FileNotFoundError:
        print("Error: File not found.")
        return
    except pd.errors.EmptyDataError:
        print("Error: File is empty.")
        return

    df['Datetime'] = pd.to_datetime(df['Date'] + ' ' + df['Time'])

    if plot_type == 'miss':
        df = df[['Datetime', 'L3MISS', 'L2MISS']]
    elif plot_type == 'hit':
        df = df[['Datetime', 'L3HIT', 'L2HIT']]
        df['L3HIT'] = df['L3HIT'] * 100
        df['L2HIT'] = df['L2HIT'] * 100
    else:
        print("Error: Invalid plot type.")
        return
    
    plt.figure(figsize=(10, 6))
    df.plot.area(x='Datetime', y=df.columns[1:], stacked=False)
    if plot_type == 'miss':
        plt.ylabel("Cache Misses")
    else:
        plt.ylabel("Cache Hit Percentage (%)")
    plt.xlabel("Time")
    plt.title("Cache Hits and Misses Over Time")
    plt.grid(True)
    plt.xticks(rotation=45)
    plt.tight_layout()

    output_filename = os.path.splitext(filename)[0] + '_graph.png'
    plt.savefig(output_filename)
    plt.show()

    print(f"Graph saved as: {output_filename}")

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("Usage: python script.py <filename.csv> <plot_type> (hit or miss)")
        sys.exit(1)

    filename = sys.argv[1]
    plot_type = sys.argv[2]
    plot_cache_hits(filename, plot_type)
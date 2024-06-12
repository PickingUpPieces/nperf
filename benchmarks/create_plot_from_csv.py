import argparse
import os
import csv
import matplotlib.pyplot as plt
import logging
# Needed for heatmap
import pandas as pd
from scipy import stats
import seaborn as sns
import numpy as np
import ast

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
PATH_TO_RESULTS_FOLDER = 'results/'


def parse_results_file(results_file):
    results = []

    with open(results_file, 'r') as file:
        reader = csv.DictReader(file)
        current_test_name = ""
        test = []
        for row in reader:
            this_test_name = row.get('test_name')
            if this_test_name != current_test_name:
                logging.info("New test found %s (old test: %s), add the old test to the results list and start a new one", this_test_name, current_test_name)
                current_test_name = this_test_name
                if test != []:
                    results.append(test)
                test = []

            test.append(row)
        results.append(test)

    logging.info('Read %s test results', len(results))
    return results

def generate_area_chart(x: str, y: str, data, chart_title, results_file, add_labels=False, rm_filename=False):
    # Iterate over list of data and add plot for every list
    for test in data:
        x_values = [float(row[x]) for row in test]
        y_values = [float(row[y]) for row in test]
        test_name = test[0]['test_name']

        if add_labels:
            for i in range(len(x_values)):
                value = "{:.0f}".format(y_values[i])
                plt.annotate(value, (x_values[i], y_values[i]), textcoords="offset points", xytext=(0,10), ha='center')

        plt.plot(x_values, y_values, label=test_name, marker='o')

    plt.xlabel(x)
    plt.ylabel(y)
    plt.ylim(bottom=0)  # Set the start of y-axis to 0
    if not rm_filename:
        plt.text(0.99, 0.5, "data: " + os.path.basename(results_file), ha='center', va='center', rotation=90, transform=plt.gcf().transFigure, fontsize=8)
    plt.title(chart_title)
    plt.legend()
    
    plt.savefig(PATH_TO_RESULTS_FOLDER + chart_title + '_area.png')
    logging.info('Saved plot to %s_area.png', chart_title)
    plt.close()

def generate_heatmap(x: str, y: str, test_name, data, chart_title, results_file, rm_filename=False):
    logging.debug('Generating heatmap for %s', test_name)
    heatmap_data = []

    logging.debug('Data: %s', data)
    # data is a list of list, where each list is a test

    for test in data:
        for run in test:
            logging.debug('Test: %s', run)
            if run['test_name'] == test_name:
                y_values = ast.literal_eval(run[y])
                for key, val in y_values.items():
                    heatmap_data.append({
                        x: run[x],
                        'Utilization': key,
                        'Value': val
                    })

    # parse heatmap data
    logging.debug('Heatmap data: %s', heatmap_data)
    
    # Create a DataFrame from the heatmap_data list
    df = pd.DataFrame(heatmap_data)
    
    # Sort x and y keys
    df[x] = df[x].astype(int)
    df['Utilization'] = df['Utilization'].astype(int)
    df = df.sort_values(by=[x, 'Utilization'])

    logging.debug('DataFrame: %s', df)

    # Devide each value through the thread_amount to get the value per thread
    if x.startswith("amount_threads"):
        df['Value'] = df.apply(lambda row: row['Value'] / row[x], axis=1)

    logging.debug('DataFrame: %s', df)

    # use log, otherwise the values are too big
    df['Value'] = np.log(df['Value'])

    logging.debug('DataFrame: %s', df)

    # Pivot the DataFrame to get the heatmap data
    pivot_table = df.pivot(index='Utilization', columns=x, values='Value').fillna(0)
    logging.debug('Pivot Table: %s', pivot_table)

    # Generate heatmap
    plt.figure(figsize=(10, 8))
    heatmap = sns.heatmap(pivot_table, cmap="YlGnBu", linewidths=.5, fmt='g')
    plt.xlabel(x)
    plt.ylabel(y)
    if not rm_filename:
        plt.text(0.99, 0.5, "data: " + os.path.basename(results_file), ha='center', va='center', rotation=90, transform=plt.gcf().transFigure, fontsize=8)
    plt.title(chart_title)
    plt.savefig(PATH_TO_RESULTS_FOLDER + chart_title + '_heatmap.png')
    logging.info('Saved plot to %s_heatmap.png', chart_title)
    plt.close()


def generate_bar_chart(y: str, data, test_name: str, results_file, rm_filename=False):
    # Map every row in the data as a bar with the y value
    logging.debug("Generating bar chart for %s with data %s", y, data)
    y_values = [float(row[y]) for row in data]
    # Enumerate every bar on the x Axis with the run_name of the specific row
    x_values = [str(row['run_name']) for row in data]
    
    # Generate bar chart
    plt.bar(x_values, y_values)
    plt.xlabel('Run Name')
    plt.ylabel(y)
    if not rm_filename:
        plt.text(0.99, 0.5, "data: " + os.path.basename(results_file), ha='center', va='center', rotation=90, transform=plt.gcf().transFigure, fontsize=8)
    plt.title(test_name)
    plt.savefig(PATH_TO_RESULTS_FOLDER + test_name + '_bar.png')
    logging.info('Saved plot to %s_bar.png', test_name)
    plt.close()
    

def get_median_result(results):
    if len(results) == 1:
        return results[0]

    array = []
    for (server_result, client_result) in results:
        array.append(server_result["data_rate_gbit"])

    logging.debug("Array of results: %s", array)

    # Calculate z-scores for each result in the array https://en.wikipedia.org/wiki/Standard_score
    zscore = (stats.zscore(array))
    logging.debug("Z-scores: %s", zscore)

    # Map each z-score in the array which is greater than 1.4/-1.4 to None
    array = [array[i] if zscore[i] < 1.4 and zscore[i] > -1.4 else None for i in range(len(array))]
    filtered_arr = [x for x in array if x is not None]
    logging.debug("Array with outliers removed: %s", filtered_arr)

    # Get the index of the median value in the original array
    median_index = find_closest_to_median_index(filtered_arr)
    logging.debug("Median index: %s", median_index)

    # Find the index of the median value in the original array
    median_index = array.index(filtered_arr[median_index])

    # Return median result
    logging.debug("Returning median result: %s", results[median_index])
    return results[median_index]

def find_closest_to_median_index(arr):
    # Check if array is empty; Otherwise argmin fails
    if not arr:
        return None
    # Calculate the median and find the index of the closest value
    closest_index = np.argmin(np.abs(np.array(arr) - np.median(arr)))
    return closest_index
    
def main():
    logging.debug('Starting main function')

    parser = argparse.ArgumentParser(description='Plot generation for nperf benchmarks.')
    parser.add_argument('results_file', help='Path to the CSV file to get the results.')
    parser.add_argument('chart_name', default="Benchmark", help='Name of the generated chart')
    parser.add_argument('x_axis_param', default="run_name", help='Name of the x-axis parameter')
    parser.add_argument('y_axis_param', help='Name of the y-axis parameter')
    parser.add_argument('--test_name', help='Name of the specific test to generate the heatmap for')
    parser.add_argument('type', default="area", help='Type of graph to generate (area, bar, heat)')
    parser.add_argument('-l', action="store_true", help='Add labels to data points')
    parser.add_argument('--rm-filename', action="store_true", help='Add the results file name to the graph')
    args = parser.parse_args()

    logging.info('Reading results file: %s', args.results_file)
    results = parse_results_file(args.results_file)
    logging.debug('Results: %s', results)

    if args.type == 'area':
        generate_area_chart(args.x_axis_param, args.y_axis_param, results, args.chart_name, args.results_file, args.l, args.rm_filename)
    elif args.type == 'bar':
        for test in results:
            generate_bar_chart(args.y_axis_param, test, test[0]["test_name"], results_file, args.rm_filename)
    elif args.type == 'heat':
        generate_heatmap(args.x_axis_param, args.y_axis_param, args.test_name, results, args.chart_name, args.results_file, args.rm_filename)

if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

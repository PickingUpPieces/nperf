import argparse
import matplotlib.pyplot as plt
import logging
import csv

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

def generate_area_chart(x: str, y: str, data, chart_title, add_labels=False):
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
    plt.title(chart_title)
    plt.legend()
    
    plt.savefig(PATH_TO_RESULTS_FOLDER + chart_title + '_area.png')
    logging.info('Saved plot to %s_area.png', chart_title)
    plt.close()


def generate_bar_chart(y: str, data, test_name: str):
    # Map every row in the data as a bar with the y value
    logging.debug("Generating bar chart for %s with data %s", y, data)
    y_values = [float(row[y]) for row in data]
    # Enumerate every bar on the x Axis with the run_name of the specific row
    x_values = [str(row['run_name']) for row in data]
    
    # Generate bar chart
    plt.bar(x_values, y_values)
    plt.xlabel('Run Name')
    plt.ylabel(y)
    plt.title(test_name)
    plt.savefig(PATH_TO_RESULTS_FOLDER + test_name + '_bar.png')
    logging.info('Saved plot to %s_bar.png', test_name)
    plt.close()
    

def main():
    logging.debug('Starting main function')

    parser = argparse.ArgumentParser(description='Plot generation for nperf benchmarks.')
    parser.add_argument('results_file', help='Path to the CSV file to get the results.')
    parser.add_argument('chart_name', default="Benchmark", help='Name of the generated chart')
    parser.add_argument('x_axis_param', default="run_name", help='Name of the x-axis parameter')
    parser.add_argument('y_axis_param', help='Name of the y-axis parameter')
    parser.add_argument('type', default="area", help='Type of graph to generate (area, bar)')
    parser.add_argument('-l', action="store_true", help='Add labels to data points')
    args = parser.parse_args()

    logging.info('Reading results file: %s', args.results_file)
    results = parse_results_file(args.results_file)
    logging.debug('Results: %s', results)

    if args.type == 'area':
        generate_area_chart(args.x_axis_param, args.y_axis_param, results, args.chart_name, args.l)
    elif args.type == 'bar':
        for test in results:
            generate_bar_chart(args.y_axis_param, test, test[0]["test_name"])

if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

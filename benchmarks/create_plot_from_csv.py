import argparse
import matplotlib.pyplot as plt
import logging
import csv

logging.basicConfig(level=logging.DEBUG , format='%(asctime)s - %(levelname)s - %(message)s')


def parse_results_file(results_file):
    results = []

    with open(results_file, 'r') as file:
        reader = csv.DictReader(file)
        for row in reader:
            results.append(row)

    return results


def generate_area_chart(x, y, data, testname):
    x_values = [float(row[x]) for row in data]
    y_values = [float(row[y]) for row in data]
    
    plt.plot(x_values, y_values, label=testname, marker='o')
    plt.xlabel(x)
    plt.ylabel(y)
    plt.title('Benchmark')
    plt.legend()
    
    plt.savefig(testname + '.png')
    plt.close()


def generate_bar_chart(y, data, testname):
    # Map every row in the data as a bar with the y value
    logging.debug("Generating bar chart for %s with data %s", y, data)
    y_values = [float(row[y]) for row in data]
    # Enumerate every bar on the x Axis with the run_number of the specific row
    x_values = [i for i, _ in enumerate(data)]
    
    # Generate bar chart
    plt.bar(x_values, y_values)
    plt.xlabel('Run Number')
    plt.ylabel(y)
    plt.title('Benchmark')
    plt.savefig(testname + '_bar.png')
    plt.close()
    

def main():
    logging.debug('Starting main function')

    parser = argparse.ArgumentParser(description='Plot generation for nperf benchmarks.')
    parser.add_argument('results_file', help='Path to the CSV file to get the results.')
    parser.add_argument('test_name', default="test", help='Name of the test')
    parser.add_argument('x_axis_param', help='Name of the x-axis parameter')
    parser.add_argument('y_axis_param', help='Name of the y-axis parameter')
    parser.add_argument('type', help='Type of graph to generate (area, bar)')
    args = parser.parse_args()

    logging.info('Reading results file: %s', args.results_file)
    results = parse_results_file(args.results_file)
    logging.info('Read %d test results', len(results))
    logging.debug('Results: %s', results)

    if args.type == 'area':
        generate_area_chart(args.x_axis_param, args.y_axis_param, results, args.test_name)
    elif args.type == 'bar':
        generate_bar_chart(args.y_axis_param, results, args.test_name)

if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

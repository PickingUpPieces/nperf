from datetime import datetime
import json
import os
import subprocess
import csv
import argparse
import json
import time
import logging
import numpy as np
import scipy.stats as stats

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
PATH_TO_RESULTS_FOLDER = 'results/'
PATH_TO_NPERF_REPO = '/home_stud/picking/repos/nperf'
#PATH_TO_NPERF_REPO = '/opt/nperf'
PATH_TO_NPERF_BIN = PATH_TO_NPERF_REPO + '/target/release/nperf'

def parse_config_file(json_file_path):
    with open(json_file_path, 'r') as json_file:
        data = json.load(json_file)

    logging.debug('Read test config: %s', data)

    global_parameters = data.pop('parameters', data)
    logging.debug('Global parameters: %s', global_parameters)
    repetitions = global_parameters.pop('repetitions', 1)

    test_configs = []

    for test_name, test_runs in data.items():
        logging.debug('Processing test: %s', test_name)
        test_config = {
            'test_name': test_name,
            'runs': [],
        }

        for run_name, run_config in test_runs.items():
            logging.debug('Processing run "%s" with config: %s', run_name, run_config)

            # If a parameter is not set in the run, use the global parameter
            run_config_client = {**global_parameters, **run_config['client']}
            run_config_server = {**global_parameters, **run_config['server']}
            run = {
                'run_name': run_name,
                'repetitions': run_config.get('repetitions', repetitions),
                'client': run_config_client,
                'server': run_config_server 
            }
            test_config["runs"].append(run)

        test_configs.append(test_config)

    return test_configs


def run_test(run_config):
    logging.debug('Running test with config: %s', run_config)

    server_command = [PATH_TO_NPERF_BIN, 'server', '--json']
    
    for k, v in run_config["server"].items():
        if v != False:
            if v == True:
                server_command.append(f'--{k}')
            else:
                server_command.append(f'--{k}')
                server_command.append(f'{v}')
    
    logging.debug('Starting server with command: %s', ' '.join(server_command))
    server_process = subprocess.Popen(server_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={'RUST_LOG': 'error'})

    # Give the server some time to start
    time.sleep(1)

    # Build client command
    client_command = [PATH_TO_NPERF_BIN, 'client', '--json']
    
    for k, v in run_config["client"].items():
        if v != False:
            if v == True:
                client_command.append(f'--{k}')
            else:
                client_command.append(f'--{k}')
                client_command.append(f'{v}')
    
    logging.debug('Starting client with command: %s', ' '.join(client_command))
    client_process = subprocess.Popen(client_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={'RUST_LOG': 'error'})

    # Wait for the client to finish
    client_output, client_error = client_process.communicate()
    if client_output:
        logging.debug('Client output: %s', client_output.decode())
    if client_error:
        logging.error('Client error: %s', client_error.decode())

    # Give the server some time to finish
    time.sleep(2)

    # Check if the server finished as well
    if server_process.poll() is None:
        logging.error('Server did not finish, retrying test')
        server_process.kill()
        return None
    
    server_output, server_error = server_process.communicate()
    if client_output:
        logging.debug('Server output: %s', server_output.decode())
    if client_error:
        logging.error('Server error: %s', server_error.decode())

    client_results = json.loads(client_output)
    server_results = json.loads(server_output)
    
    # Add run_name to results
    server_results['run_name'] = run_config['run_name']
    client_results['run_name'] = run_config['run_name']

    logging.info('Returning results: %s', server_results)
    return (server_results, client_results)
    

def write_results_to_csv(test_results, test_name, csv_file_path):
    # FIXME: If new measurement parameters are added, the header should be updated
    header = ['test_name', 'run_number', 'run_name', 'amount_threads_client', 'amount_threads_server', 'amount_used_ports_server', 'test_runtime_length', 'datagram_size', 'packet_buffer_size', 'exchange_function', 'io_model', 'total_data_gbyte', 'amount_datagrams', 'amount_data_bytes', 'amount_reordered_datagrams', 'amount_duplicated_datagrams', 'amount_omitted_datagrams', 'amount_syscalls', 'amount_io_model_syscalls', 'data_rate_gbit', 'packet_loss', 'nonblocking', 'ip_fragmentation', 'gso', 'gro']
    file_exists = os.path.isfile(csv_file_path)

    with open(csv_file_path, 'a', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=header)

        if not file_exists:
            writer.writeheader()


        # FIXME: Add new measurement parameter as a new column here
        for index, (server_result, client_result) in enumerate(test_results):
            if client_result['parameter']['socket_options']['gso'] is None:
                client_result['parameter']['socket_options']['gso'] = False

            row = {
                'test_name': test_name,
                'run_number': index,
                'run_name': server_result['run_name'],
                'amount_threads_client': client_result['parameter']['amount_threads'],
                'amount_threads_server': server_result['parameter']['amount_threads'],
                'amount_used_ports_server': server_result['parameter']['amount_ports'],
                'test_runtime_length': server_result['parameter']['test_runtime_length'],
                'datagram_size': server_result['parameter']['datagram_size'],
                'packet_buffer_size': server_result['parameter']['packet_buffer_size'],
                'exchange_function': server_result['parameter']['exchange_function'],
                'io_model': server_result['parameter']['io_model'],
                'total_data_gbyte': server_result['total_data_gbyte'],
                'amount_datagrams': server_result['amount_datagrams'],
                'amount_data_bytes': server_result['amount_data_bytes'],
                'amount_reordered_datagrams': server_result['amount_reordered_datagrams'],
                'amount_duplicated_datagrams': server_result['amount_duplicated_datagrams'],
                'amount_omitted_datagrams': server_result['amount_omitted_datagrams'],
                'data_rate_gbit': server_result['data_rate_gbit'],
                'amount_syscalls': server_result['amount_syscalls'],
                'amount_io_model_syscalls': server_result['amount_io_model_syscalls'],
                'packet_loss': server_result['packet_loss'],
                'nonblocking': server_result['parameter']['socket_options']['nonblocking'],
                'ip_fragmentation': client_result['parameter']['socket_options']['ip_fragmentation'],
                'gso': client_result['parameter']['socket_options']['gso'],
                'gro': server_result['parameter']['socket_options']['gro']
            }
            writer.writerow(row)

def get_file_name(test_name):
    timestamp = int(time.time())
    dt_object = datetime.fromtimestamp(timestamp)
    formatted_datetime = dt_object.strftime("%m-%d-%H:%M")
    return f"{test_name}-{formatted_datetime}.csv"

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
    # Calculate the median and find the index of the closest value
    closest_index = np.argmin(np.abs(np.array(arr) - np.median(arr)))
    return closest_index

def main():
    logging.debug('Starting main function')

    logging.info('Compiling binary in release mode.')
    subprocess.run(['cargo', 'build', '--release'], check=True, cwd=PATH_TO_NPERF_REPO)

    parser = argparse.ArgumentParser(description='Benchmark nperf.')
    parser.add_argument('config_file', help='Path to the JSON configuration file')
    parser.add_argument('results_file', nargs='?', default='test_results.csv', help='Path to the CSV file to write the results')
    parser.add_argument('-m', action="store_true", help='Merge results data into one file')
    args = parser.parse_args()

    logging.debug('Parsed arguments: %s', args)
    logging.info('Reading config file: %s', args.config_file)
    test_configs = parse_config_file(args.config_file)
    logging.info('Read %d test configs', len(test_configs))

    csv_file_name = args.results_file

    if args.results_file == 'test_results.csv' and args.m is True:
        csv_file_name = get_file_name(test_configs[0]["test_name"])
        
    test_results = []

    # Create directory for test results
    os.makedirs(PATH_TO_RESULTS_FOLDER, exist_ok=True)

    for config in test_configs:
        logging.debug('Processing config: %s', config)
        test_name = config["test_name"]

        if args.results_file == 'test_results.csv' and args.m is False:
            csv_file_name = get_file_name(test_name)

        for run in config["runs"]:
            logging.info('Run config: %s', run)
            run_results = []
            for _ in range(run["repetitions"]):
                for _ in range(0,2): # Retries, in case of an error
                    result = run_test(run)
                    if result is not None: 
                        run_results.append(result)
                        break
            test_results.append(get_median_result(run_results))

        logging.info('Writing results to CSV file: %s', csv_file_name)
        write_results_to_csv(test_results, test_name, PATH_TO_RESULTS_FOLDER + csv_file_name)
        test_results = []


if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

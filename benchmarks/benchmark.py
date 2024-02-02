from datetime import datetime
import json
import os
import subprocess
import csv
import argparse
import json
import time
import logging

logging.basicConfig(level=logging.INFO , format='%(asctime)s - %(levelname)s - %(message)s')
PATH_TO_NPERF_BIN = '/opt/nperf/target/release/nperf'


def parse_config_file(json_file_path):
    with open(json_file_path, 'r') as json_file:
        data = json.load(json_file)

    logging.debug('Read test config: %s', data)

    test_configs = []

    for test_name, test_runs in data.items():
        logging.debug('Processing test: %s', test_name)
        test_config = {
            'test_name': test_name,
            'runs': [],
        }

        for run_number, run_config in test_runs.items():
            logging.debug('Processing run number %s with config: %s', run_number, run_config)
            run = {
                'repetitions': run_config['repetitions'],
                'client': run_config['client'],
                'server': run_config['server']
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
    time.sleep(1)

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

    # TODO: Merge results from client and server, where necessary
    client_results = json.loads(client_output)
    server_results = json.loads(server_output)
    
    return server_results
    

def write_results_to_csv(test_results, test_name, csv_file_path):
    header = ['test_name', 'run_number', 'test_runtime_length', 'datagram_size', 'packet_buffer_size', 'exchange_function', 'io_model', 'total_data_gbyte', 'amount_datagrams', 'amount_data_bytes', 'amount_reordered_datagrams', 'amount_duplicated_datagrams', 'amount_omitted_datagrams', 'data_rate_gbit', 'packet_loss', 'nonblocking', 'without_ip_frag', 'gso', 'gro']
    file_exists = os.path.isfile(csv_file_path)

    with open(csv_file_path, 'a', newline='') as csvfile:
        writer = csv.DictWriter(csvfile, fieldnames=header)

        if not file_exists:
            writer.writeheader()

        for index, result in enumerate(test_results):
            row = {
                'test_name': test_name,
                'run_number': index,
                'test_runtime_length': result['parameter']['test_runtime_length'],
                'datagram_size': result['parameter']['datagram_size'],
                'packet_buffer_size': result['parameter']['packet_buffer_size'],
                'exchange_function': result['parameter']['exchange_function'],
                'io_model': result['parameter']['io_model'],
                'total_data_gbyte': result['total_data_gbyte'],
                'amount_datagrams': result['amount_datagrams'],
                'amount_data_bytes': result['amount_data_bytes'],
                'amount_reordered_datagrams': result['amount_reordered_datagrams'],
                'amount_duplicated_datagrams': result['amount_duplicated_datagrams'],
                'amount_omitted_datagrams': result['amount_omitted_datagrams'],
                'data_rate_gbit': result['data_rate_gbit'],
                'packet_loss': result['packet_loss'],
                'nonblocking': result['parameter']['socket_options']['nonblocking'],
                'without_ip_frag': result['parameter']['socket_options']['without_ip_frag'],
                'gso': result['parameter']['socket_options']['gso'][0],
                'gro': result['parameter']['socket_options']['gro']
            }
            writer.writerow(row)


def main():
    logging.debug('Starting main function')

    parser = argparse.ArgumentParser(description='Benchmark nperf.')
    parser.add_argument('config_file', nargs='?', default='test.config', help='Path to the JSON configuration file.')
    parser.add_argument('results_file', nargs='?', default='test_results.csv', help='Path to the CSV file to write the results.')
    args = parser.parse_args()

    logging.info('Reading config file: %s', args.config_file)
    test_configs = parse_config_file(args.config_file)
    logging.info('Read %d test configs', len(test_configs))

    for config in test_configs:
        logging.debug('Processing config: %s', config)
        test_name = config["test_name"]

        csv_file_name = args.results_file
        if csv_file_name == 'test_results.csv':
            timestamp = int(time.time())
            dt_object = datetime.fromtimestamp(timestamp)
            formatted_datetime = dt_object.strftime("%m-%d-%H:%M")
            csv_file_name = f"{test_name}-{formatted_datetime}.csv"

        test_results = []
        
        for run in config["runs"]:
            logging.info('Run config: %s', run)
            result = run_test(run)
            test_results.append(result)

        logging.info('Writing results to CSV file: %s', csv_file_name)
        write_results_to_csv(test_results, test_name, csv_file_name)


if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

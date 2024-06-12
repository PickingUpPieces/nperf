from datetime import datetime
import json
import os
import subprocess
import csv
import argparse
import json
import time
import logging

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

def load_json(json_str):
    try:
        return json.loads(json_str)
    except json.JSONDecodeError:
        return None

def run_test(run_config, test_name, file_name):
    logging.debug('Running test with config: %s', run_config)

    time.sleep(5) # Short timeout to give system some time

    # Replace with file name
    server_command = [PATH_TO_NPERF_BIN, 'server', '--output-format=file', f'--output-file-path={PATH_TO_RESULTS_FOLDER}server-{file_name}', f'--label-test={test_name}', f'--label-run={run_config["run_name"]}']
    
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
    client_command = [PATH_TO_NPERF_BIN, 'client', '--output-format=file', f'--output-file-path={PATH_TO_RESULTS_FOLDER}client-{file_name}', f'--label-test={test_name}', f'--label-run={run_config["run_name"]}']
    
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

    logging.debug('Returning results: %s', server_output)
    return (server_output, client_output)
 

def get_file_name(file_name):
    timestamp = int(time.time())
    dt_object = datetime.fromtimestamp(timestamp)
    formatted_datetime = dt_object.strftime("%m-%d-%H:%M")
    return f"{file_name}-{formatted_datetime}.csv"

def main():
    logging.debug('Starting main function')

    logging.info('Compiling binary in release mode.')
    subprocess.run(['cargo', 'build', '--release'], check=True, cwd=PATH_TO_NPERF_REPO)

    parser = argparse.ArgumentParser(description='Benchmark nperf.')
    parser.add_argument('config_file', help='Path to the JSON configuration file')
    parser.add_argument('results_file', nargs='?', default='test_results.csv', help='Path to the CSV file to write the results')
    args = parser.parse_args()

    logging.debug('Parsed arguments: %s', args)
    logging.info('Reading config file: %s', args.config_file)

    test_configs = parse_config_file(args.config_file)
    logging.info('Read %d test configs', len(test_configs))

    csv_file_name = args.results_file

    if args.results_file == 'test_results.csv':
        csv_file_name = get_file_name(os.path.splitext(os.path.basename(args.config_file))[0])
    
    logging.info('Config file name: %s', csv_file_name)
    test_results = []

    # Create directory for test results
    os.makedirs(PATH_TO_RESULTS_FOLDER, exist_ok=True)

    for config in test_configs:
        logging.debug('Processing config: %s', config)
        test_name = config["test_name"]

        for run in config["runs"]:
            logging.info('Run config: %s', run)
            run_results = []
            for i in range(run["repetitions"]):
                logging.info('Run repetition: %i/%i', i+1, run["repetitions"])
                for _ in range(0,3): # Retries, in case of an error
                    server_output, client_output = run_test(run, test_name, csv_file_name)
                    if server_output is '': 
                        logging.warning('Result is not empty: %s', server_output)
                        run_results.append(server_output)
                    elif client_output is '':
                        logging.warning('Client Output is not empty: %s', client_output)
                        run_results.append(client_output)
                    break
            if len(run_results) != 0:
                test_results.append(run_results)

        logging.info('Checking results for errors')
        logging.info('Results: %s', test_results)
        # TODO: Check if there were any errors in output
        test_results = []


if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

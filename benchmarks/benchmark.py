from datetime import datetime
import json
import os
import subprocess
import argparse
import json
import threading
import time
import logging
import yaml

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
PATH_TO_RESULTS_FOLDER = 'results/'
PATH_TO_NPERF_REPO = '/home_stud/picking/repos/nperf'
#PATH_TO_NPERF_REPO = '/opt/nperf'
PATH_TO_NPERF_BIN = PATH_TO_NPERF_REPO + '/target/release/nperf'
nperf_binary = PATH_TO_NPERF_BIN

def parse_config_file(json_file_path):
    with open(json_file_path, 'r') as json_file:
        data = json.load(json_file)

    logging.debug('Read test config: %s', data)

    global_parameters = data.pop('parameters', data)
    logging.debug('Global parameters: %s', global_parameters)
    repetitions = global_parameters.pop('repetitions', 1)

    test_configs = []

    for test_name, test_runs in data.items():
        logging.debug('Processing test %s', test_name)
        test_parameters = test_runs.pop('parameters', {})
        logging.debug('Test specific parameters: %s', test_parameters)

        test_config = {
            'test_name': test_name,
            'runs': [],
        }

        for run_name, run_config in test_runs.items():
            logging.debug('Processing run "%s" with config: %s', run_name, run_config)

            # Add test parameters first
            run_config_client = {**test_parameters, **run_config['client']}
            run_config_server = {**test_parameters, **run_config['server']}

            # Add global parameters at last
            run_config_client = {**global_parameters, **run_config_client}
            run_config_server = {**global_parameters, **run_config_server}

            run = {
                'run_name': run_name,
                'repetitions': run_config.get('repetitions', repetitions),
                'client': run_config_client,
                'server': run_config_server 
            }
            logging.debug('Complete run config: %s', run)

            test_config["runs"].append(run)

        test_configs.append(test_config)

    return test_configs

def load_json(json_str):
    try:
        return json.loads(json_str)
    except json.JSONDecodeError:
        return None


def run_test_client(run_config, test_name, file_name) -> bool:
    logging.debug('Running client test with config: %s', run_config)

    # Build client command
    client_command = [nperf_binary, 'client', '--output-format=file', f'--output-file-path={PATH_TO_RESULTS_FOLDER}client-{file_name}', f'--label-test={test_name}', f'--label-run={run_config["run_name"]}']
    
    for k, v in run_config["client"].items():
        if v is not False:
            if v is True:
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

    return True

def run_test_server(run_config, test_name, file_name) -> True:
    logging.debug('Running server test with config: %s', run_config)
    # Replace with file name
    server_command = [nperf_binary, 'server', '--output-format=file', f'--output-file-path={PATH_TO_RESULTS_FOLDER}server-{file_name}', f'--label-test={test_name}', f'--label-run={run_config["run_name"]}']
    
    for k, v in run_config['server'].items():
        if v is not False:
            if v is True:
                server_command.append(f'--{k}')
            else:
                server_command.append(f'--{k}')
                server_command.append(f'{v}')
    
    logging.debug('Starting server with command: %s', ' '.join(server_command))
    server_process = subprocess.Popen(server_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={'RUST_LOG': 'error'})
    server_output, server_error = server_process.communicate(timeout=(run_config["client"]["time"] + 10)) # Add 10 seconds as buffer to the client time

    if server_output:
        logging.debug('Server output: %s', server_output.decode())
    if server_error:
        logging.error('Server error: %s', server_error.decode())

    # Check if the server finished 
    if server_process.poll() is None:
        logging.error('Server did not finish, retrying test')
        server_process.kill()
        return False
    
    logging.debug('Returning results: %s', server_output)
    return True
 

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
    parser.add_argument('config_file', nargs='?', help='Path to the JSON configuration file')
    parser.add_argument('results_file', nargs='?', default='test_results.csv', help='Path to the CSV file to write the results')
    parser.add_argument('--nperf_bin', default=PATH_TO_NPERF_BIN, help='Path to the nperf binary')
    parser.add_argument('--yaml', help='Path to the YAML configuration file')  # Add YAML config file option
    # Remote measurements only available over yaml config file
    # client_ssh, server_ssh, client_interface, server_interface

    args = parser.parse_args()

    # If YAML config is provided, parse it and use its parameters
    if args.yaml:
        with open(args.yaml, 'r') as yaml_file:
            yaml_config = yaml.safe_load(yaml_file)
            # Use values from YAML config, potentially overriding other command-line arguments
            nperf_binary = yaml_config.get('nperf_bin', PATH_TO_NPERF_BIN)
            csv_file_name = yaml_config.get('results_file', 'test_results.csv')
            config_file = yaml_config.get('config_file')
    else:
        nperf_binary = args.nperf_bin
        config_file = args.config_file
        if config_file is None:
            logging.error("Config file must be supplied!")
            return
        csv_file_name = args.results_file

    if csv_file_name == 'test_results.csv':
        csv_file_name = get_file_name(os.path.splitext(os.path.basename(config_file))[0])

    logging.debug('Parsed arguments: %s', args)
    logging.info('Reading config file: %s', config_file)
    logging.info('Input file name: %s', csv_file_name)

    test_configs = parse_config_file(config_file)
    logging.info('Read %d test configs', len(test_configs))

    test_results = []

    # Create directory for test results
    os.makedirs(PATH_TO_RESULTS_FOLDER, exist_ok=True)

    for config in test_configs:
        logging.debug('Processing config: %s', config)
        test_name = config["test_name"]

        for run in config["runs"]:
            logging.info('Run config: %s', run)

            for i in range(run["repetitions"]):
                logging.info('Run repetition: %i/%i', i+1, run["repetitions"])
                for _ in range(0,3): # Retries, in case of an error
                    logging.info('Wait for some seconds so system under test can normalize...')
                    time.sleep(3)
                    logging.info('Starting test run')
                    server_thread = threading.Thread(target=run_test_server, args=(run, test_name, csv_file_name))
                    client_thread = threading.Thread(target=run_test_client, args=(run, test_name, csv_file_name))
                    server_thread.start()
                    time.sleep(1) # Wait for server to be ready
                    client_thread.start()

                    client_thread.join(timeout=run["client"]["time"] + 15)
                    server_thread.join(timeout=run["client"]["time"] + 15)

                    break

        logging.info('Results: %s', test_results)
        # TODO: Check if there were any errors in output
        test_results = []

    logging.info(f"Results stored in: {PATH_TO_RESULTS_FOLDER}server-{csv_file_name}")
    logging.info(f"Results stored in: {PATH_TO_RESULTS_FOLDER}client-{csv_file_name}")


if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

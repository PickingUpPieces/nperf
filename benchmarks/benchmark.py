from concurrent.futures import ThreadPoolExecutor
from datetime import datetime
import json
import os
import signal
import subprocess
import argparse
import json
import time
import logging
import yaml

logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')
PATH_TO_RESULTS_FOLDER = 'results/'
PATH_TO_NPERF_REPO = '/home_stud/picking/repos/nperf'
#PATH_TO_NPERF_REPO = '/opt/nperf'
PATH_TO_NPERF_BIN = PATH_TO_NPERF_REPO + '/target/release/nperf'
MAX_FAILED_ATTEMPTS = 3

def parse_config_file(json_file_path: str) -> list[dict]:
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


def run_test_client(run_config, test_name: str, file_name: str, ssh_client: str) -> bool:
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
    
    command_str = ' '.join(client_command)
    logging.debug('Starting client with command: %s', command_str)

    env_vars = os.environ.copy()
    env_vars['RUST_LOG'] = 'error'
    # Ensure SSH_AUTH_SOCK is forwarded if available
    if 'SSH_AUTH_SOCK' in os.environ:
        env_vars['SSH_AUTH_SOCK'] = os.environ['SSH_AUTH_SOCK']

    if ssh_client:
        # Modify the command to be executed over SSH
        ssh_command = f"ssh {ssh_client} '{command_str}'"
        client_process = subprocess.Popen(ssh_command, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env_vars)
    else:
        # Execute command locally
        client_process = subprocess.Popen(client_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env_vars)

    # Wait for the client to finish
    client_output, client_error = client_process.communicate()
    if client_output:
        logging.debug('Client output: %s', client_output.decode())
    if client_error:
        logging.error('Client error: %s', client_error.decode())
        log_file_name = file_name.replace('.csv', '.log')
        log_file_path = f'{PATH_TO_RESULTS_FOLDER}client-{log_file_name}'
        
        with open(log_file_path, 'a') as log_file:
            log_file.write("Test: " + test_name + " Run: " + run_config["run_name"] + '\n')
            log_file.write("Config: " + str(run_config) + '\n')
            log_file.write(client_error.decode())

        return False

    return True

def run_test_server(run_config, test_name: str, file_name: str, ssh_server: str) -> bool:
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
    
    command_str = ' '.join(server_command)
    logging.debug('Starting server with command: %s', command_str)

    env_vars = os.environ.copy()
    env_vars['RUST_LOG'] = 'error'
    # Ensure SSH_AUTH_SOCK is forwarded if available
    if 'SSH_AUTH_SOCK' in os.environ:
        env_vars['SSH_AUTH_SOCK'] = os.environ['SSH_AUTH_SOCK']

    if ssh_server:
        # Modify the command to be executed over SSH
        ssh_command = f"ssh {ssh_server} 'sudo {command_str}'"
        server_process = subprocess.Popen(ssh_command, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env_vars)
    else:
        # Execute command locally
        server_process = subprocess.Popen(server_command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env={'RUST_LOG': 'error'})

    # Wait for the server to finish
    try:
        server_output, server_error = server_process.communicate(timeout=(run_config["client"]["time"] + 10)) # Add 10 seconds as buffer to the client time
    except subprocess.TimeoutExpired:
        logging.error('Server process timed out')
        return False

    if server_output:
        logging.debug('Server output: %s', server_output.decode())
    if server_error:
        logging.error('Server error: %s', server_error.decode())
        log_file_name = file_name.replace('.csv', '.log')
        log_file_path = f'{PATH_TO_RESULTS_FOLDER}server-{log_file_name}'
        
        with open(log_file_path, 'a') as log_file:
            log_file.write("Test: " + test_name + " Run: " + run_config["run_name"] + '\n')
            log_file.write("Config: " + str(run_config) + '\n')
            log_file.write(server_error.decode())
        return False

    # Check if the server finished 
    if server_process.poll() is None:
        logging.error('Server did not finish, retrying test')
        server_process.kill()
        return False
    
    logging.debug('Returning results: %s', server_output)
    return True
 
def test_ssh_connection(ssh_address):
    try:
        result = subprocess.run(['ssh', ssh_address, 'echo ok'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10)
        if result.stdout.decode().strip() == 'ok':
            logging.info(f"SSH connection to {ssh_address} successful.")
            return True
        else:
            logging.error(f"SSH connection to {ssh_address} failed. Error: {result.stderr.decode()}")
            return False
    except subprocess.TimeoutExpired:
        logging.error(f"SSH connection to {ssh_address} timed out.")
        return False
    except Exception as e:
        logging.error(f"Error testing SSH connection to {ssh_address}: {e}")
        return False

def get_file_name(file_name: str) -> str:
    timestamp = int(time.time())
    dt_object = datetime.fromtimestamp(timestamp)
    formatted_datetime = dt_object.strftime("%m-%d-%H:%M")
    return f"{file_name}-{formatted_datetime}.csv"

def kill_server_process(port, ssh_server):
    logging.info(f'Killing server process on port {port}')
    try:
        # Find process listening on the given port
        if ssh_server is None:
            result = subprocess.run(['sudo', 'lsof', '-i', f':{port}', '-t'], capture_output=True, text=True)
        else:
            result = subprocess.run(['ssh', ssh_server, 'sudo', 'lsof', '-i', f':{port}', '-t'], capture_output=True, text=True)
            
        pids = result.stdout.strip().split('\n')
        for pid in pids:
            if pid:
                logging.info(f'Killing process {pid} on port {port}')
                if ssh_server is None:
                    os.kill(int(pid), signal.SIGTERM)
                else:
                    subprocess.run(['ssh', ssh_server, f'sudo kill -9 {pid}'], capture_output=True, text=True)
    except Exception as e:
        logging.error(f'Failed to kill process on port {port}: {e}')

def main():
    logging.debug('Starting main function')

    parser = argparse.ArgumentParser(description='Benchmark nperf.')
    parser.add_argument('config_file', nargs='?', help='Path to the JSON configuration file')
    parser.add_argument('results_file', nargs='?', default='test_results.csv', help='Path to the CSV file to write the results')
    parser.add_argument('--nperf_bin', default=PATH_TO_NPERF_BIN, help='Path to the nperf binary')
    parser.add_argument('--yaml', help='Path to the YAML configuration file')  # Add YAML config file option
    # Remote measurements only available over yaml config file
    # client_ssh, server_ssh

    args = parser.parse_args()

    global nperf_binary

    # If YAML config is provided, parse it and use its parameters
    if args.yaml:
        with open(args.yaml, 'r') as yaml_file:
            yaml_config = yaml.safe_load(yaml_file)
            # Use values from YAML config, potentially overriding other command-line arguments
            nperf_binary = yaml_config.get('nperf_bin', PATH_TO_NPERF_BIN)
            csv_file_name = yaml_config.get('results_file', 'test_results.csv')
            config_file = yaml_config.get('config_file')
            ssh_client = yaml_config.get('ssh_client', None)
            ssh_server = yaml_config.get('ssh_server', None)

    else:
        nperf_binary = args.nperf_bin
        config_file = args.config_file
        ssh_client = None
        ssh_server = None
        if config_file is None:
            logging.error("Config file must be supplied!")
            return
        csv_file_name = args.results_file

    if csv_file_name == 'test_results.csv':
        csv_file_name = get_file_name(os.path.splitext(os.path.basename(config_file))[0])

    logging.debug('Parsed arguments: %s', args)
    logging.info('Using nPerf Binary %s', nperf_binary)
    logging.info('Reading config file: %s', config_file)
    logging.info('Input file name: %s', csv_file_name)

    test_configs = parse_config_file(config_file)
    logging.info('Read %d test configs', len(test_configs))

    # Check SSH connections if applicable
    if ssh_client is not None:
        logging.debug("Testing SSH connection to client...")
        if not test_ssh_connection(ssh_client):
            logging.error("SSH connection to client failed. Exiting.")
            exit(1)

    if ssh_server is not None:
        logging.debug("Testing SSH connection to server...")
        if not test_ssh_connection(ssh_server):
            logging.error("SSH connection to server failed. Exiting.")
            exit(1)

    if ssh_client is None and ssh_server is None:
        logging.info('Compiling binary in release mode.')
        subprocess.run(['cargo', 'build', '--release'], check=True, cwd=PATH_TO_NPERF_REPO)

    # Create directory for test results
    os.makedirs(PATH_TO_RESULTS_FOLDER, exist_ok=True)

    for config in test_configs:
        logging.debug('Processing config: %s', config)
        test_name = config["test_name"]

        for run in config["runs"]:
            logging.info('Run config: %s', run)
            thread_timeout = run["client"]["time"] + 15

            for i in range(run["repetitions"]):
                logging.info('Run repetition: %i/%i', i+1, run["repetitions"])
                failed_attempts = 0  # Initialize failed attempts counter
                for _ in range(0,MAX_FAILED_ATTEMPTS): # Retries, in case of an error
                    logging.info('Wait for some seconds so system under test can normalize...')
                    time.sleep(3)
                    logging.info('Starting test run')
                    with ThreadPoolExecutor(max_workers=2) as executor:
                        future_server = executor.submit(run_test_server, run, test_name, csv_file_name, ssh_server)
                        time.sleep(1) # Wait for server to be ready
                        future_client = executor.submit(run_test_client, run, test_name, csv_file_name, ssh_client)

                        if future_server.result(timeout=thread_timeout) and future_client.result(timeout=thread_timeout):
                            logging.info(f'Test run {run["run_name"]} finished successfully')
                            break
                        else:
                            logging.error(f'Test run {run["run_name"]} failed, retrying')
                            kill_server_process(run["server"]["port"], ssh_server)
                            failed_attempts += 1

                if failed_attempts == MAX_FAILED_ATTEMPTS:
                    logging.error('Maximum number of failed attempts reached. Dont execute next repetition.')
                    break

    logging.info(f"Results stored in: {PATH_TO_RESULTS_FOLDER}server-{csv_file_name}")
    logging.info(f"Results stored in: {PATH_TO_RESULTS_FOLDER}client-{csv_file_name}")


if __name__ == '__main__':
    logging.info('Starting script')
    main()
    logging.info('Script finished')

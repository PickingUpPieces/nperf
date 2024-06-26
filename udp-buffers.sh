#!/bin/bash
# This script is used to get/set the max UDP buffer sizes

default_wmem_max=212992
default_rmem_max=212992

# Set the new values for kernel parameters
new_wmem_max=26214400 # 25MB
new_rmem_max=26214400 # 25MB
new_netdev_max_backlog=5000
new_txqueuelen=10000

# Check the current values of kernel parameters
current_wmem_max=$(sysctl -n net.core.wmem_max)
current_rmem_max=$(sysctl -n net.core.rmem_max)
current_netdev_max_backlog=$(sysctl -n net.core.netdev_max_backlog)
current_txqueuelen=$(sysctl -n txqueuelen)

echo "!!!SCRIPT MUST BE EXECUTED AS ROOT!!!"
echo "Current values:"
echo "net.core.wmem_max: $current_wmem_max"
echo "net.core.rmem_max: $current_rmem_max"
echo "net.core.netdev_max_backlog: $current_netdev_max_backlog"
echo "txqueuelen: $current_txqueuelen"

read -p "Do you want to change the values? (y/n): " choice

if [[ $choice == "y" ]]; then
    echo "Setting new values..."
    sysctl -w net.core.wmem_max=$new_wmem_max
    sysctl -w net.core.rmem_max=$new_rmem_max
    sysctl -w net.core.netdev_max_backlog=$new_netdev_max_backlog
    sysctl -w txqueuelen=$new_txqueuelen

    echo "New values set successfully!"
else
    echo "Values will be left as they currently are."
fi

# Check if current values differ from default
if [ "$current_wmem_max" != "$default_wmem_max" ] || [ "$current_rmem_max" != "$default_rmem_max" ]; then
  # Ask the user if they want to reset to default
  read -p "Current buffer values differ from default. Do you want to reset to default? (y/n): " choice 

  if [ "$choice" = "y" ]; then
    # Reset buffer values to default
    sysctl -w net.core.wmem_max=$default_wmem_max
    sysctl -w net.core.rmem_max=$default_rmem_max
  fi
fi
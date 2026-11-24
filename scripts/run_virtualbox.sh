#!/usr/bin/env bash
set -e

VM_NAME="TitaniumMicroOS"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT_DIR}"

ISO_PATH="${ROOT_DIR}/dist/micro-os.iso"
VDI_PATH="${ROOT_DIR}/dist/micro-os-bios.vdi"
LOG_DIR="${ROOT_DIR}/traces"
LOG_FILE="${LOG_DIR}/qemu-boot.log"

if command -v VBoxManage &> /dev/null; then
    echo "=== Shutting down any previous VM instance ==="
    VBoxManage controlvm "${VM_NAME}" poweroff 2>/dev/null || true
    sleep 1
    VBoxManage unregistervm "${VM_NAME}" --delete 2>/dev/null || true
    rm -rf "$HOME/VirtualBox VMs/${VM_NAME}" 2>/dev/null || true

    mkdir -p "${LOG_DIR}"
    touch "${LOG_FILE}"

    if [ ! -f "${ISO_PATH}" ]; then
        echo "Error: ISO not found at ${ISO_PATH}"
        exit 1
    fi

    echo "=== Creating and configuring VirtualBox VM '${VM_NAME}' ==="
    VBoxManage createvm --name "${VM_NAME}" --ostype "Other_64" --register
    sleep 1
    VBoxManage modifyvm "${VM_NAME}" --memory 128 --firmware bios
    VBoxManage setextradata "${VM_NAME}" "VBoxInternal2/EnableX86OnArm" 1 2>/dev/null || true

    # Storage controller setup
    VBoxManage storagectl "${VM_NAME}" --name "IDE Controller" --add ide 2>/dev/null || true

    # Attach bootable VDI hard disk
    VBoxManage storageattach "${VM_NAME}" --storagectl "IDE Controller" --port 0 --device 0 --type hdd --medium "${VDI_PATH}"

    # Attach bootable ISO DVD drive
    VBoxManage storageattach "${VM_NAME}" --storagectl "IDE Controller" --port 1 --device 0 --type dvddrive --medium "${ISO_PATH}"

    # Boot order: Disk first, then DVD
    VBoxManage modifyvm "${VM_NAME}" --boot1 disk --boot2 dvd --boot3 none --boot4 none

    # Serial port redirection to trace log
    VBoxManage modifyvm "${VM_NAME}" --uart1 0x3F8 4 --uartmode1 file "${LOG_FILE}"

    echo "=== Starting VirtualBox GUI Window ==="
    VBoxManage startvm "${VM_NAME}" --type gui
    echo "=== VirtualBox VM successfully launched! ==="
else
    echo "Warning: VBoxManage not installed on host."
    exit 3
fi

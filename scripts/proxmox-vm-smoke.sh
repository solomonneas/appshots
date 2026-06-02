#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Create a disposable Proxmox VM for appshots Linux distro smoke tests.

Dry-run by default. Run this on a Proxmox VE host.

Usage:
  scripts/proxmox-vm-smoke.sh [options]
  scripts/proxmox-vm-smoke.sh --apply [options]
  scripts/proxmox-vm-smoke.sh --destroy --vmid 130

Options:
  --apply                 Run commands instead of printing them.
  --destroy               Stop and destroy the VM.
  --vmid ID               VMID to create or destroy. Default: 130.
  --name NAME             VM name. Default: appshots-debian13-smoke.
  --storage STORAGE       Proxmox storage for disks/cloud-init. Default: local-lvm.
  --bridge BRIDGE         Network bridge. Default: vmbr0.
  --ssh-key PATH          SSH public key for cloud-init. Default: ~/.ssh/id_rsa.pub.
  --ci-user USER          Cloud-init user. Default: appshots.
  --memory MiB            VM memory. Default: 2048.
  --cores N               VM CPU cores. Default: 2.
  --disk-size SIZE        Disk resize target. Default: 16G.
  --image-url URL         Debian cloud image URL.
  --image-path PATH       Local image cache path.

After the VM boots, find its IP from Proxmox or DHCP and run:
  scripts/linux-vm-smoke.sh appshots@<ip-or-host>

References:
  Proxmox Cloud-Init Support: https://pve.proxmox.com/wiki/Cloud-Init_Support
USAGE
}

apply=0
destroy=0
vmid=130
name="appshots-debian13-smoke"
storage="local-lvm"
bridge="vmbr0"
ssh_key="${HOME}/.ssh/id_rsa.pub"
ci_user="appshots"
memory=2048
cores=2
disk_size="16G"
image_url="https://cloud.debian.org/images/cloud/trixie/latest/debian-13-genericcloud-amd64.qcow2"
image_path="/var/lib/vz/template/qcow2/debian-13-genericcloud-amd64.qcow2"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply) apply=1; shift ;;
    --destroy) destroy=1; shift ;;
    --vmid) vmid="$2"; shift 2 ;;
    --name) name="$2"; shift 2 ;;
    --storage) storage="$2"; shift 2 ;;
    --bridge) bridge="$2"; shift 2 ;;
    --ssh-key) ssh_key="$2"; shift 2 ;;
    --ci-user) ci_user="$2"; shift 2 ;;
    --memory) memory="$2"; shift 2 ;;
    --cores) cores="$2"; shift 2 ;;
    --disk-size) disk_size="$2"; shift 2 ;;
    --image-url) image_url="$2"; shift 2 ;;
    --image-path) image_path="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
done

run() {
  printf '+'
  printf ' %q' "$@"
  printf '\n'
  if [[ "$apply" -eq 1 ]]; then
    "$@"
  fi
}

require_host_tools() {
  for cmd in qm curl install; do
    command -v "$cmd" >/dev/null || {
      echo "missing required command on Proxmox host: $cmd" >&2
      exit 1
    }
  done
}

if [[ "$destroy" -eq 1 ]]; then
  if [[ "$apply" -eq 1 ]]; then
    require_host_tools
  fi
  run qm stop "$vmid" --skiplock 1
  run qm destroy "$vmid" --purge 1
  exit 0
fi

if [[ "$apply" -eq 1 ]]; then
  require_host_tools
fi

if [[ "$apply" -eq 1 && ! -r "$ssh_key" ]]; then
  echo "SSH public key not found: $ssh_key" >&2
  echo "Pass --ssh-key /path/to/key.pub." >&2
  exit 1
fi

if [[ "$apply" -eq 1 ]] && qm status "$vmid" >/dev/null 2>&1; then
  echo "VMID already exists: $vmid" >&2
  echo "Choose another --vmid or destroy the existing VM explicitly." >&2
  exit 1
fi

cat <<INFO
Proxmox VM smoke test plan
  distro: Debian 13 cloud image
  vmid: $vmid
  name: $name
  storage: $storage
  bridge: $bridge
  resources: ${cores} core(s), ${memory} MiB RAM, ${disk_size} disk
  mode: $([[ "$apply" -eq 1 ]] && echo apply || echo dry-run)

INFO

run install -d "$(dirname "$image_path")"
if [[ ! -f "$image_path" ]]; then
  run curl -fL "$image_url" -o "$image_path"
fi

run qm create "$vmid" \
  --name "$name" \
  --memory "$memory" \
  --cores "$cores" \
  --net0 "virtio,bridge=${bridge}" \
  --scsihw virtio-scsi-pci \
  --serial0 socket \
  --vga serial0 \
  --agent enabled=1

run qm set "$vmid" --scsi0 "${storage}:0,import-from=${image_path}"
run qm set "$vmid" --ide2 "${storage}:cloudinit"
run qm set "$vmid" --boot order=scsi0
run qm set "$vmid" --ciuser "$ci_user"
run qm set "$vmid" --sshkey "$ssh_key"
run qm set "$vmid" --ipconfig0 ip=dhcp
run qm disk resize "$vmid" scsi0 "$disk_size"
run qm start "$vmid"

cat <<NEXT

Next:
  1. Wait for cloud-init to finish.
  2. Find the VM IP from Proxmox, DHCP, or guest agent if available.
  3. Run:
       scripts/linux-vm-smoke.sh ${ci_user}@<ip-or-host>
  4. Destroy when done:
       scripts/proxmox-vm-smoke.sh --apply --destroy --vmid ${vmid}
NEXT

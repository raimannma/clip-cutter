import argparse
import subprocess
import sys
from time import sleep
from typing import List, Union

TOKEN = ""


def install(package):
    subprocess.check_call([sys.executable, "-m", "pip", "install", package])


try:
    from hcloud import Client
    from hcloud.server_types import ServerType
    from hcloud.images import Image
    from hcloud.servers import CreateServerResponse, BoundServer
    from hcloud.ssh_keys import SSHKey
except ImportError:
    install("hcloud")
    from hcloud import Client
    from hcloud.server_types import ServerType
    from hcloud.images import Image
    from hcloud.servers import CreateServerResponse, BoundServer
    from hcloud.ssh_keys import SSHKey

try:
    from paramiko.client import SSHClient, AutoAddPolicy
except ImportError:
    install("paramiko")
    from paramiko.client import SSHClient, AutoAddPolicy


def create_server(client: Client) -> CreateServerResponse:
    print("Creating server")
    return client.servers.create(
        name="clip-cutter",
        server_type=ServerType(name="cpx21"),
        image=Image(name="debian-12"),
        ssh_keys=[SSHKey(name="clip-cutter")],
    )


def destroy_server(client: Client, server: BoundServer):
    print("Destroying server")
    client.servers.delete(server)


def run_commands(ssh_client: SSHClient, commands: Union[str, List[str]]):
    if not isinstance(commands, list):
        commands = [commands]
    command = " && ".join(commands)
    print("Running command", command)
    _, stdout, stderr = ssh_client.exec_command(command, bufsize=0)
    for line in stdout:
        print(line, end="")


def update_server(ssh_client: SSHClient):
    run_commands(ssh_client, "apt-get update && apt-get upgrade -y")


def install_apt_packages(ssh_client: SSHClient):
    packages = [
        "ca-certificates",
        "curl",
        "jq",
        "rclone",
        "python3",
        "python3-pip",
        "pipx",
    ]
    run_commands(ssh_client, f"apt-get install -y {' '.join(packages)}")


def install_pip_packages(ssh_client: SSHClient):
    run_commands(
        ssh_client,
        [
            f"pipx install twitch-dl",
            f"pipx install youtube-dl",
        ],
    )


def install_docker(ssh_client: SSHClient):
    run_commands(
        ssh_client,
        [
            "install -m 0755 -d /etc/apt/keyrings",
            "curl -fsSL https://download.docker.com/linux/debian/gpg -o /etc/apt/keyrings/docker.asc",
            'echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/debian $(. /etc/os-release && echo "$VERSION_CODENAME") stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null',
            "apt-get update",
            "apt-get install -y docker-ce docker-ce-cli containerd.io docker-buildx-plugin docker-compose-plugin",
            "echo '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' | docker login ghcr.io/raimannma/clip-cutter --username raimannma --password-stdin",
        ],
    )


def add_rclone_config(ssh_client: SSHClient):
    config = """
    [Nextcloud]
    type = webdav
    url = https://nx46418.your-storageshare.de/remote.php/dav/files/Admin
    vendor = nextcloud
    user = Admin
    pass = cBMdg9oT9VopqabTnoeAa8tuNACYVy_oemHr1JUgss9ErzuZppuzLogCfA62
    """
    run_commands(
        ssh_client,
        [
            "mkdir -p /root/.config/rclone",
            f"echo '{config}' > /root/.config/rclone/rclone.conf",
        ],
    )


def setup_server(ssh_client: SSHClient):
    print("Setting up server")
    run_commands(
        ssh_client,
        [
            "export DEBIAN_FRONTEND=noninteractive",
            "echo -e 'export PATH=$PATH:/root/.local/bin' >> .bashrc",
        ],
    )
    update_server(ssh_client)
    install_apt_packages(ssh_client)
    install_pip_packages(ssh_client)
    install_docker(ssh_client)
    add_rclone_config(ssh_client)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run clip cutter")
    parser.add_argument(
        "--do-not-create-server",
        action="store_true",
        help="Do not create a new server",
        type=bool,
    )
    parser.add_argument(
        "--do-not-destroy-server",
        action="store_true",
        help="Do not destroy the server after running",
        type=bool,
    )
    args = parser.parse_args()

    client = Client(token=TOKEN)

    servers = client.servers.get_all()
    server = servers[0] if servers else None

    if server is None:
        if args.do_not_create_server:
            print("No server found and do-not-create-server flag set, exiting")
            sys.exit(1)
        server = create_server(client).server
        print("Created server", server.name)

        slept_for = 0
        while server.status != "running":
            print(f"Waiting for server to start, slept for {slept_for} seconds, current status: {server.status}")
            sleep(5)
            slept_for += 5
            server = client.servers.get_by_id(server.id)
        sleep(30)

    ip_address = server.public_net.ipv4.ip
    print("Server IP", ip_address)

    ssh_client = SSHClient()
    ssh_client.set_missing_host_key_policy(AutoAddPolicy())
    ssh_client.connect(ip_address, username="root", key_filename="id_rsa_clip_cutter")

    setup_server(ssh_client)

    print("Moving process_all.sh to server")
    sftp = ssh_client.open_sftp()
    sftp.put("process_all.sh", "/root/process_all.sh")
    sftp.put(".env", "/root/.env")
    sftp.close()

    run_commands(ssh_client, "bash /root/process_all.sh")

    print("Finished processing all")
    ssh_client.close()

    if not args.do_not_destroy_server:
        destroy_server(client, server)
        print("Destroyed server", server.name)

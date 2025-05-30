import os
import sys

try:
    # Python 3.11+
    import tomllib
    def load_toml_config(file_path):
        with open(file_path, 'rb') as f:
            return tomllib.load(f)
except ImportError:
    # Python < 3.11
    import toml
    def load_toml_config(file_path):
        with open(file_path, 'r', encoding='utf-8') as f:
            return toml.load(f)

try:
    config = load_toml_config('pyproject.toml')
    app_version = config['project']['version']
    print(f"Project version read: {app_version}")

    github_env_file = os.getenv('GITHUB_ENV')
    if github_env_file:
        with open(github_env_file, 'a', encoding='utf-8') as env_f:
            env_f.write(f"APP_VERSION={app_version}\n")
        print(f"APP_VERSION set to {app_version} in GITHUB_ENV")
    else:
        print("Error: GITHUB_ENV not found.", file=sys.stderr)
        sys.exit(1)
except Exception as e:
    print(f"Error in get_version.py: {e}", file=sys.stderr)
    sys.exit(1)
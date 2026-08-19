#!/usr/bin/env python

"""
Impulcifer 명령줄 진입점
"""

from _impulcifer_entrypoint import prefer_distribution_root

prefer_distribution_root()

from impulcifer import create_cli, main  # noqa: E402


def entry_point():
    """명령줄에서 실행 시 진입점 함수"""
    args = create_cli()
    main(**args)


if __name__ == "__main__":
    entry_point() 
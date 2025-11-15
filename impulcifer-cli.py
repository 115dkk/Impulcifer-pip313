#!/usr/bin/env python

"""
Impulcifer 명령줄 진입점
"""

from impulcifer import main, create_cli

def entry_point():
    """명령줄에서 실행 시 진입점 함수"""
    args = create_cli()
    main(**args)

if __name__ == "__main__":
    entry_point() 
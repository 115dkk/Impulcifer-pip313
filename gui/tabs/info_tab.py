#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""Info tab for the modern GUI.

Shows version, contributors, system info, and project links. Moved from
``gui/modern_gui.py`` without behavioural changes.
"""

import os
import platform
import sys
import webbrowser
from pathlib import Path

import customtkinter as ctk

import impulcifer
from core.parallel_processing import get_python_threading_info
from updater.updater_core import is_pip_environment, is_velopack_environment


class InfoTab:
    def __init__(self, app):
        self.app = app
        self.loc = app.loc
        self.fonts = app.fonts
        self.tabview = app.tabview
        self.root = app.root
        self._build()

    def _build(self):
        """Create Info tab with version, contributors, system info, and links"""
        tab = self.tabview.tab(self.loc.get('tab_info'))
        tab.grid_columnconfigure(0, weight=1)
        tab.grid_rowconfigure(0, weight=1)

        # Create scrollable frame
        scroll = ctk.CTkScrollableFrame(tab, corner_radius=10)
        scroll.grid(row=0, column=0, sticky="nsew", padx=10, pady=10)
        scroll.grid_columnconfigure(0, weight=1)

        section_row = 0
        label_font = self.fonts['label']
        value_font = self.fonts['value']
        heading_font = self.fonts['heading']

        # === About Section ===
        about_frame = ctk.CTkFrame(scroll, corner_radius=0)
        about_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        about_frame.grid_columnconfigure(1, weight=1)
        section_row += 1

        r = 0
        ctk.CTkLabel(about_frame, text=self.loc.get('section_about'), font=heading_font
                      ).grid(row=r, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        # Version
        r += 1
        ctk.CTkLabel(about_frame, text=self.loc.get('label_version'), font=label_font
                      ).grid(row=r, column=0, sticky="w", padx=15, pady=5)
        ctk.CTkLabel(about_frame, text=impulcifer.__version__, font=value_font
                      ).grid(row=r, column=1, sticky="w", padx=5, pady=5)

        # Installation type
        r += 1
        ctk.CTkLabel(about_frame, text=self.loc.get('label_installation'), font=label_font
                      ).grid(row=r, column=0, sticky="w", padx=15, pady=5)
        if is_velopack_environment():
            install_text = self.loc.get('info_install_velopack')
        elif is_pip_environment():
            install_text = self.loc.get('info_install_pip')
        else:
            install_text = self.loc.get('info_install_dev')
        ctk.CTkLabel(about_frame, text=install_text, font=value_font
                      ).grid(row=r, column=1, sticky="w", padx=5, pady=5)

        # License button
        r += 1

        def open_license():
            # Nuitka 빌드에서는 LICENSE가 License.txt로 리네임되어 번들됨
            candidates = [
                Path(sys.executable).parent / 'License.txt',   # Nuitka 빌드
                Path(__file__).parent.parent.parent / 'LICENSE',       # 개발 환경
                Path(__file__).parent.parent.parent / 'License.txt',   # 혹시 모를 경우
            ]
            license_path = None
            for p in candidates:
                if p.exists():
                    license_path = p
                    break

            if license_path:
                if platform.system() == 'Windows':
                    os.startfile(license_path)
                elif platform.system() == 'Darwin':
                    os.system(f'open "{license_path}"')
                else:
                    os.system(f'xdg-open "{license_path}"')
            else:
                webbrowser.open("https://github.com/115dkk/Impulcifer-pip313/blob/main/LICENSE")

        ctk.CTkButton(about_frame, text=self.loc.get('button_view_license'),
                       command=open_license, width=200
                       ).grid(row=r, column=0, columnspan=2, sticky="w", padx=15, pady=(10, 5))

        # Bug report button
        r += 1
        ctk.CTkButton(about_frame, text=self.loc.get('button_report_bug'),
                       command=lambda: webbrowser.open("https://github.com/115dkk/Impulcifer-pip313/issues/new"),
                       width=200
                       ).grid(row=r, column=0, columnspan=2, sticky="w", padx=15, pady=(5, 15))

        # === Contributors Section ===
        contrib_frame = ctk.CTkFrame(scroll, corner_radius=0)
        contrib_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        contrib_frame.grid_columnconfigure(1, weight=1)
        section_row += 1

        ctk.CTkLabel(contrib_frame, text=self.loc.get('section_contributors'), font=heading_font
                      ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        contributors = [
            ("Jaakko Pasanen", self.loc.get('contributor_role_original')),
            ("115dkk", self.loc.get('contributor_role_py313')),
            ("LionLion123", self.loc.get('contributor_role_contributor')),
            ("SDC (DCinside)", self.loc.get('contributor_role_contributor')),
        ]
        for i, (name, role) in enumerate(contributors):
            ctk.CTkLabel(contrib_frame, text=name, font=value_font
                          ).grid(row=i + 1, column=0, sticky="w", padx=15, pady=3)
            ctk.CTkLabel(contrib_frame, text=role, font=label_font
                          ).grid(row=i + 1, column=1, sticky="w", padx=5, pady=3)
        # bottom padding for last contributor
        contrib_frame.grid_rowconfigure(len(contributors) + 1, minsize=10)

        # === System Information Section ===
        sys_frame = ctk.CTkFrame(scroll, corner_radius=0)
        sys_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        sys_frame.grid_columnconfigure(1, weight=1)
        section_row += 1

        ctk.CTkLabel(sys_frame, text=self.loc.get('section_system_info'), font=heading_font
                      ).grid(row=0, column=0, columnspan=2, sticky="w", padx=15, pady=(15, 10))

        try:
            threading_info = get_python_threading_info()
        except Exception:
            threading_info = {}

        py_ver = threading_info.get('python_version', f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}")
        os_name = f"{platform.system()} {platform.release()}"
        cpu_cores = str(threading_info.get('cpu_count', os.cpu_count() or '?'))
        optimal_workers = str(threading_info.get('optimal_workers', '?'))

        gil_raw = threading_info.get('gil_enabled', 'unknown (pre-3.14)')
        if gil_raw is True:
            gil_text = self.loc.get('info_gil_enabled')
        elif gil_raw is False:
            gil_text = self.loc.get('info_gil_disabled')
        else:
            gil_text = self.loc.get('info_gil_unknown')

        sys_items = [
            (self.loc.get('label_python'), py_ver),
            (self.loc.get('label_os'), os_name),
            (self.loc.get('label_cpu_cores'), cpu_cores),
            (self.loc.get('label_gil_status'), gil_text),
            (self.loc.get('label_optimal_workers'), optimal_workers),
        ]
        for i, (label, value) in enumerate(sys_items):
            ctk.CTkLabel(sys_frame, text=label, font=label_font
                          ).grid(row=i + 1, column=0, sticky="w", padx=15, pady=3)
            ctk.CTkLabel(sys_frame, text=value, font=value_font
                          ).grid(row=i + 1, column=1, sticky="w", padx=5, pady=3)
        sys_frame.grid_rowconfigure(len(sys_items) + 1, minsize=10)

        # === Project Links Section ===
        links_frame = ctk.CTkFrame(scroll, corner_radius=0)
        links_frame.grid(row=section_row, column=0, sticky="ew", padx=10, pady=10)
        links_frame.grid_columnconfigure(0, weight=1)
        section_row += 1

        ctk.CTkLabel(links_frame, text=self.loc.get('section_project_links'), font=heading_font
                      ).grid(row=0, column=0, sticky="w", padx=15, pady=(15, 10))

        ctk.CTkButton(links_frame, text=self.loc.get('button_original_repo'),
                       command=lambda: webbrowser.open("https://github.com/jaakkopasanen/Impulcifer"),
                       width=280
                       ).grid(row=1, column=0, sticky="w", padx=15, pady=(5, 5))

        ctk.CTkButton(links_frame, text=self.loc.get('button_fork_repo'),
                       command=lambda: webbrowser.open("https://github.com/115dkk/Impulcifer-pip313"),
                       width=280
                       ).grid(row=2, column=0, sticky="w", padx=15, pady=(5, 15))

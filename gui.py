def main_gui():
	import os
	import tkinter
	import re
	from tkinter import Tk, Frame, Label, Button, Entry, StringVar, DoubleVar, IntVar, BooleanVar, Toplevel, Checkbutton, OptionMenu, Scrollbar, Text, END, DISABLED, NORMAL, HORIZONTAL, VERTICAL, W, E, N, S, Canvas
	from tkinter.filedialog import askdirectory, askopenfilename, asksaveasfilename
	from tkinter.messagebox import showinfo
	import recorder, impulcifer
	import sounddevice

	#tooltip for widgets
	class ToolTip(object):
		def __init__(self, widget, text='widget info'):
			self.waittime = 500     #miliseconds
			self.wraplength = 180   #pixels
			self.widget = widget
			self.text = text
			self.widget.bind("<Enter>", self.enter)
			self.widget.bind("<Leave>", self.leave)
			self.widget.bind("<ButtonPress>", self.leave)
			self.id = None
			self.tw = None
		def enter(self, event=None):
			self.schedule()
		def leave(self, event=None):
			self.unschedule()
			self.hidetip()
		def schedule(self):
			self.unschedule()
			self.id = self.widget.after(self.waittime, self.showtip)
		def unschedule(self):
			id = self.id
			self.id = None
			if id:
				self.widget.after_cancel(id)
		def showtip(self, event=None):
			x = y = 0
			x, y, cx, cy = self.widget.bbox("insert")
			x += self.widget.winfo_rootx() + 25
			y += self.widget.winfo_rooty() + 20
			# creates a toplevel window
			self.tw = Toplevel(self.widget)
			# Leaves only the label and removes the app window
			self.tw.wm_overrideredirect(True)
			self.tw.wm_geometry(f"+{x}+{y}")
			label = Label(self.tw, text=self.text, justify='left',
						  background="#ffffff", relief='solid', borderwidth=1,
						  wraplength = self.wraplength)
			label.pack(ipadx=1)
		def hidetip(self):
			tw = self.tw
			self.tw= None
			if tw:
				tw.destroy()

	#decimal entry validator
	def validate_double(inp):
		if not inp or inp == '-':
			return True
		try:
			float(inp)
		except:
			return False
		return True

	#integer entry validator
	def validate_int(inp):
		if not inp:
			return True
		try:
			int(inp)
		except:
			return False
		if len(inp) > 5: #limit chars to 5
			return False
		if '-' in inp:
			return False
		return True

	#open dir dialog
	def opendir(var):
		path = askdirectory(initialdir=os.path.dirname(var.get()))
		if not path:
			return
		path = os.path.abspath(path) #make all separators the correct one
		path = path.replace(os.getcwd() + os.path.sep, '') #prefer relative paths when possible
		var.set(path)


	#open file dialog
	def openfile(var, filetypes):
		path = askopenfilename(initialdir=os.path.dirname(var.get()), initialfile=os.path.basename(var.get()), filetypes=filetypes)
		if not path:
			return
		path = os.path.abspath(path)
		path = path.replace(os.getcwd() + os.path.sep, '')
		var.set(path)

	#save file dialog
	def savefile(var):
		path = asksaveasfilename(initialdir=os.path.dirname(var.get()), initialfile=os.path.basename(var.get()), defaultextension=".wav", filetypes=(('WAV file', '*.wav'), ('All files', '*.*')))
		if not path:
			return
		path = os.path.abspath(path)
		path = path.replace(os.getcwd() + os.path.sep, '')
		var.set(path)

	#pack widget into canvas
	def pack(widget, current_pos, current_maxwidth, current_maxheight, samerow=False):
		if not samerow:
			current_pos[1] += widget.winfo_reqheight() + 5
			current_pos[0] = 10
		widget.place(x=current_pos[0], y=current_pos[1], anchor=W)
		widgetpos = (current_pos[0], current_pos[1])
		current_pos[0] += widget.winfo_reqwidth()
		current_maxwidth = max(current_maxwidth, current_pos[0])
		current_maxheight = current_pos[1] + 20
		root.update()
		return widgetpos, current_pos, current_maxwidth, current_maxheight

	#RECORDER WINDOW
	root = Tk()

	root.title('Recorder')
	root.resizable(False, False)
	canvas1 = Canvas(root)

	pos = [0, 0]
	maxwidth = 0
	maxheight = 0

	#refresh record window
	def refresh1(init=False):
		host_apis = {}
		i = 0
		for host in sounddevice.query_hostapis():
			host_apis[i] = host['name']
			i += 1

		host_api_optionmenu['menu'].delete(0, 'end')
		for host in host_apis.values():
			host_api_optionmenu['menu'].add_command(label=host, command=tkinter._setit(host_api, host))

		if not host_apis:
			host_api.set('')
		elif init and 'Windows DirectSound' in host_apis.values():
			host_api.set('Windows DirectSound')
		elif host_api.get() not in host_apis.values():
			host_api.set(host_apis[0])

		output_devices = []
		input_devices = []
		for device in sounddevice.query_devices():
			if host_apis[device['hostapi']] == host_api.get():
				if device['max_output_channels'] > 0:
					output_devices.append(device['name'])
				elif device['max_input_channels'] > 0:
					input_devices.append(device['name'])
		output_device_optionmenu['menu'].delete(0, 'end')
		input_device_optionmenu['menu'].delete(0, 'end')
		for device in output_devices:
			output_device_optionmenu['menu'].add_command(label=device, command=tkinter._setit(output_device, device))
		for device in input_devices:
			input_device_optionmenu['menu'].add_command(label=device, command=tkinter._setit(input_device, device))
		if not output_devices:
			output_device.set('')
		elif output_device.get() not in output_devices:
			output_device.set(output_devices[0])
		if not input_devices:
			input_device.set('')
		elif input_device.get() not in input_devices:
			input_device.set(input_devices[0])

		channels_entry.config(state=NORMAL if channels_check.get() else DISABLED)

	#playback device
	output_device = StringVar()
	output_device.trace('w', lambda *args: refresh1())
	widgetpos, pos, maxwidth, maxheight = pack(Label(canvas1, text='Playback device'), pos, maxwidth, maxheight)
	output_device_optionmenu = OptionMenu(canvas1, variable=output_device, value=None, command=refresh1)
	widgetpos, pos, maxwidth, maxheight = pack(output_device_optionmenu, pos, maxwidth, maxheight, samerow=True)

	#record device
	input_device = StringVar()
	input_device.trace('w', lambda *args: refresh1())
	widgetpos, pos, maxwidth, maxheight = pack(Label(canvas1, text='Recording device'), pos, maxwidth, maxheight)
	input_device_optionmenu = OptionMenu(canvas1, variable=input_device, value=None, command=refresh1)
	widgetpos, pos, maxwidth, maxheight = pack(input_device_optionmenu, pos, maxwidth, maxheight, samerow=True)

	#host API
	widgetpos, pos, maxwidth, maxheight = pack(Label(canvas1, text='Host API'), pos, maxwidth, maxheight)
	host_api = StringVar()
	host_api.trace('w', lambda *args: refresh1())
	host_api_optionmenu = OptionMenu(canvas1, host_api, value=None, command=refresh1)
	widgetpos, pos, maxwidth, maxheight = pack(host_api_optionmenu, pos, maxwidth, maxheight, samerow=True)

	#sound file to play
	widgetpos, pos, maxwidth, maxheight = pack(Label(canvas1, text='File to play'), pos, maxwidth, maxheight)
	play = StringVar(value=os.path.join('data', 'sweep-seg-FL,FR-stereo-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
	play_entry = Entry(canvas1, textvariable=play, width=70)
	widgetpos, pos, maxwidth, maxheight = pack(play_entry, pos, maxwidth, maxheight)
	widgetpos, pos, maxwidth, maxheight = pack(Button(canvas1, text='...', command=lambda: openfile(play, (('Audio files', '*.wav'), ('All files', '*.*')))), pos, maxwidth, maxheight, samerow=True)

	#output file
	widgetpos, pos, maxwidth, maxheight = pack(Label(canvas1, text='Record to file'), pos, maxwidth, maxheight)
	record = StringVar(value=os.path.join('data', 'my_hrir', 'FL,FR.wav'))
	record_entry = Entry(canvas1, textvariable=record, width=70)
	widgetpos, pos, maxwidth, maxheight = pack(record_entry, pos, maxwidth, maxheight)
	widgetpos, pos, maxwidth, maxheight = pack(Button(canvas1, text='...', command=lambda: savefile(record)), pos, maxwidth, maxheight, samerow=True)

	#force number of channels
	channels_check = BooleanVar()
	channels_checkbutton = Checkbutton(canvas1, text="Force input channels", variable=channels_check, command=refresh1)
	widgetpos, pos, maxwidth, maxheight = pack(channels_checkbutton, pos, maxwidth, maxheight)
	ToolTip(channels_checkbutton, 'For room correction: some measurement microphones like MiniDSP UMIK-1 are seen as stereo microphones by Windows and will for that reason record a stereo file. recorder can force the capture to be one channel')
	channels = IntVar(value=1)
	channels_entry = Entry(canvas1, textvariable=channels, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos, pos, maxwidth, maxheight = pack(channels_entry, pos, maxwidth, maxheight, samerow=True)

	#append
	append = BooleanVar()
	append_check = Checkbutton(canvas1, text="Append", variable=append)
	ToolTip(append_check, 'Add track(s) to existing file. Silence will be added to end of each track to make all equal in length.')
	widgetpos, pos, maxwidth, maxheight = pack(append_check, pos, maxwidth, maxheight)

	#record button
	def recordaction():
		recorder.play_and_record(play=play_entry.get(), record=record_entry.get(), input_device=input_device.get(), output_device=output_device.get(), host_api=host_api.get(), channels=(channels.get() if channels_check.get() else 2), append=append.get())
		showinfo('', 'Recorded to ' + record_entry.get())
	widgetpos, pos, maxwidth, maxheight = pack(Button(canvas1, text='RECORD', command=recordaction), pos, maxwidth, maxheight)

	refresh1(init=True)
	root.geometry(str(maxwidth) + 'x' + str(maxheight) + '+0+0')
	canvas1.config(width=maxwidth, height=maxheight)
	canvas1.pack()
	canvas1_final_width = maxwidth

	#IMPULCIFER WINDOW
	pos2 = [0, 0]
	imp_maxwidth = 0
	imp_maxheight = 0
	window2 = Toplevel(root)
	window2.title('Impulcifer')
	canvas2 = Canvas(window2)

	#refresh impulcifer window
	def refresh2(changedpath=False):
		if changedpath:
			if os.path.exists(dir_path.get()):
				files = os.listdir(dir_path.get().strip())
				if len(files) > 100: #don't want to scan a megafolder
					return
				s = ';'.join(files)
				if re.search(r"\broom(-[A-Z]{2}(,[A-Z]{2})*-(left|right))?\.wav\b", s, re.I):
					do_room_correction_msg.set('found room wav')
					do_room_correction_msg_label.config(foreground='green')
				else:
					do_room_correction_msg.set('room wav not found!')
					do_room_correction_msg_label.config(foreground='red')
				if re.search(r'\bheadphones\.wav\b', s, re.I):
					do_headphone_compensation_msg.set('found headphones wav')
					do_headphone_compensation_msg_label.config(foreground='green')
				else:
					do_headphone_compensation_msg.set('headphones wav not found!')
					do_headphone_compensation_msg_label.config(foreground='red')
				if re.search(r"\beq(-left|-right)?\.csv\b", s, re.I):
					do_equalization_msg.set('found eq csv')
					do_equalization_msg_label.config(foreground='green')
				else:
					do_equalization_msg.set('eq csv not found!')
					do_equalization_msg_label.config(foreground='red')

		if do_room_correction.get():
			do_room_correction_msg_label.place(x=label_pos[do_room_correction_msg_label][0], y=label_pos[do_room_correction_msg_label][1], anchor=W)
		else:
			do_room_correction_msg_label.place_forget()
		if do_headphone_compensation.get():
			do_headphone_compensation_msg_label.place(x=label_pos[do_headphone_compensation_msg_label][0], y=label_pos[do_headphone_compensation_msg_label][1], anchor=W)
		else:
			do_headphone_compensation_msg_label.place_forget()
		if do_equalization.get():
			do_equalization_msg_label.place(x=label_pos[do_equalization_msg_label][0], y=label_pos[do_equalization_msg_label][1], anchor=W)
		else:
			do_equalization_msg_label.place_forget()

		specific_limit_entry.config(state=NORMAL if do_room_correction.get() else DISABLED)
		generic_limit_entry.config(state=NORMAL if do_room_correction.get() else DISABLED)
		room_target_entry.config(state=NORMAL if do_room_correction.get() else DISABLED)
		room_mic_calibration_entry.config(state=NORMAL if do_room_correction.get() else DISABLED)
		fr_combination_method_optionmenu.config(state=NORMAL if do_room_correction.get() else DISABLED)
		fs_optionmenu.config(state=NORMAL if fs_check.get() else DISABLED)
		decay_entry.config(state=DISABLED if decay_per_channel.get() else NORMAL)

		if show_adv.get():
			for widget in adv_options_pos:
				widget.place(x=adv_options_pos[widget][0], y=adv_options_pos[widget][1], anchor=W)

			if channel_balance.get() == 'number':
				channel_balance_db_entry.place(x=adv_options_pos[channel_balance_db_entry][0], y=adv_options_pos[channel_balance_db_entry][1], anchor=W)
				channel_balance_db_label.place(x=adv_options_pos[channel_balance_db_label][0], y=adv_options_pos[channel_balance_db_label][1], anchor=W)
			else:
				channel_balance_db_entry.place_forget()
				channel_balance_db_label.place_forget()

			if decay_per_channel.get():
				for i in range(7):
					decay_labels[i].place(x=adv_options_pos[decay_labels[i]][0], y=adv_options_pos[decay_labels[i]][1], anchor=W)
					decay_entries[i].place(x=adv_options_pos[decay_entries[i]][0], y=adv_options_pos[decay_entries[i]][1], anchor=W)
			else:
				for i in range(7):
					decay_labels[i].place_forget()
					decay_entries[i].place_forget()
		else:
			for widget in adv_options_pos:
				widget.place_forget()

	#your recordings
	_, pos2, imp_maxwidth, imp_maxheight = pack(Label(canvas2, text='Your recordings'), pos2, imp_maxwidth, imp_maxheight)
	dir_path = StringVar(value=os.path.join('data', 'my_hrir'))
	dir_path.trace('w', lambda *args: refresh2(changedpath=True))
	dir_path_entry = Entry(canvas2, textvariable=dir_path, width=80)
	_, pos2, imp_maxwidth, imp_maxheight = pack(dir_path_entry, pos2, imp_maxwidth, imp_maxheight)
	_, pos2, imp_maxwidth, imp_maxheight = pack(Button(canvas2, text='...', command=lambda: opendir(dir_path)), pos2, imp_maxwidth, imp_maxheight, samerow=True)

	#test signal used
	test_signal_label = Label(canvas2, text='Test signal used')
	ToolTip(test_signal_label, 'Signal used in the measurement.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(test_signal_label, pos2, imp_maxwidth, imp_maxheight)
	test_signal = StringVar(value=os.path.join('data', 'sweep-6.15s-48000Hz-32bit-2.93Hz-24000Hz.wav'))
	test_signal_entry = Entry(canvas2, textvariable=test_signal, width=80)
	_, pos2, imp_maxwidth, imp_maxheight = pack(test_signal_entry, pos2, imp_maxwidth, imp_maxheight)
	_, pos2, imp_maxwidth, imp_maxheight = pack(Button(canvas2, text='...', command=lambda: openfile(test_signal, (('Audio files', '*.wav *.pkl'), ('All files', '*.*')))), pos2, imp_maxwidth, imp_maxheight, samerow=True)

	#room correction
	label_pos = {}
	do_room_correction = BooleanVar()
	do_room_correction_checkbutton = Checkbutton(canvas2, text="Room correction ", variable=do_room_correction, command=lambda: refresh2(changedpath=True if do_room_correction.get() else False))
	ToolTip(do_room_correction_checkbutton, "Do room correction from room measurements in format room-<SPEAKERS>-<left|right>.wav located in your folder; e.g. room-FL,FR-left.wav. Generic measurements are named room.wav")
	_, pos2, imp_maxwidth, imp_maxheight = pack(do_room_correction_checkbutton, pos2, imp_maxwidth, imp_maxheight)
	do_room_correction_msg = StringVar()
	do_room_correction_msg_label = Label(canvas2, textvariable=do_room_correction_msg)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(do_room_correction_msg_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	label_pos[do_room_correction_msg_label] = widgetpos_temp
	specific_limit = IntVar(value=20000)
	specific_limit_label = Label(canvas2, text='Specific Limit (Hz)')
	ToolTip(specific_limit_label, "Upper limit for room equalization with speaker-ear specific room measurements. Equalization will drop down to 0 dB at this frequency in the leading octave.")
	_, pos2, imp_maxwidth, imp_maxheight = pack(specific_limit_label, pos2, imp_maxwidth, imp_maxheight)
	specific_limit_entry = Entry(canvas2, textvariable=specific_limit, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	_, pos2, imp_maxwidth, imp_maxheight = pack(specific_limit_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	generic_limit = IntVar(value=1000)
	genericlimitlabel = Label(canvas2, text='Generic Limit (Hz)')
	ToolTip(genericlimitlabel, "Upper limit for room equalization with generic room measurements. Equalization will drop down to 0 dB at this frequency in the leading octave.")
	_, pos2, imp_maxwidth, imp_maxheight = pack(genericlimitlabel, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	generic_limit_entry = Entry(canvas2, textvariable=generic_limit, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	_, pos2, imp_maxwidth, imp_maxheight = pack(generic_limit_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	fr_combination_method_label = Label(canvas2, text='FR combination method')
	_, pos2, imp_maxwidth, imp_maxheight = pack(fr_combination_method_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	fr_combination_methods = ['average', 'conservative']
	fr_combination_method = StringVar(value=fr_combination_methods[0])
	fr_combination_method_optionmenu = OptionMenu(canvas2, fr_combination_method, *fr_combination_methods)
	ToolTip(fr_combination_method_label, 'Method for combining frequency responses of generic room measurements if there are more than one tracks in the file. "average" will simply average the frequency responses. "conservative" will take the minimum absolute value for each frequency but only if the values in all the measurements are positive or negative at the same time.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(fr_combination_method_optionmenu, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	room_mic_calibration_label = Label(canvas2, text='Mic calibration')
	_, pos2, imp_maxwidth, imp_maxheight = pack(room_mic_calibration_label, pos2, imp_maxwidth, imp_maxheight)
	room_mic_calibration = StringVar()
	room_mic_calibration_entry = Entry(canvas2, textvariable=room_mic_calibration, width=65)
	ToolTip(room_mic_calibration_label, 'Calibration data is subtracted from the room frequency responses. Uses room-mic-calibration.txt (or csv) by default if it exists.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(room_mic_calibration_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	_, pos2, imp_maxwidth, imp_maxheight = pack(Button(canvas2, text='...', command=lambda: openfile(room_mic_calibration, (('Text files', '*.csv *.txt'), ('All files', '*.*')))), pos2, imp_maxwidth, imp_maxheight, samerow=True)
	room_target_label = Label(canvas2, text='Target Curve')
	_, pos2, imp_maxwidth, imp_maxheight = pack(room_target_label, pos2, imp_maxwidth, imp_maxheight)
	room_target = StringVar()
	room_target_entry = Entry(canvas2, textvariable=room_target, width=65)
	ToolTip(room_target_label, 'Head related impulse responses will be equalized with the difference between room response measurements and room response target. Uses room-target.txt (or csv) by default if it exists.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(room_target_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	_, pos2, imp_maxwidth, imp_maxheight = pack(Button(canvas2, text='...', command=lambda: openfile(room_target, (('Text files', '*.csv *.txt'), ('All files', '*.*')))), pos2, imp_maxwidth, imp_maxheight, samerow=True)

	#headphone compensation
	do_headphone_compensation = BooleanVar()
	do_headphone_compensation_checkbutton = Checkbutton(canvas2, text="Headphone compensation ", variable=do_headphone_compensation, command=lambda: refresh2(changedpath=True if do_headphone_compensation.get() else False))
	ToolTip(do_headphone_compensation_checkbutton, 'Equalize HRIR tracks with headphone compensation measurement headphones.wav')
	_, pos2, imp_maxwidth, imp_maxheight = pack(do_headphone_compensation_checkbutton, pos2, imp_maxwidth, imp_maxheight)
	do_headphone_compensation_msg = StringVar()
	do_headphone_compensation_msg_label = Label(canvas2, textvariable=do_headphone_compensation_msg)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(do_headphone_compensation_msg_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	label_pos[do_headphone_compensation_msg_label] = widgetpos_temp

	#headphone EQ
	do_equalization = BooleanVar()
	do_equalization_checkbutton = Checkbutton(canvas2, text="Custom EQ", variable=do_equalization, command=lambda: refresh2(changedpath=True if do_equalization.get() else False))
	ToolTip(do_equalization_checkbutton, 'Read equalization FIR filter or CSV settings from file called eq.csv in your folder. The eq file must be an AutoEQ produced result CSV file. Separate equalizations are supported with files eq-left.csv and eq-right.csv.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(do_equalization_checkbutton, pos2, imp_maxwidth, imp_maxheight)
	do_equalization_msg = StringVar()
	do_equalization_msg_label = Label(canvas2, textvariable=do_equalization_msg)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(do_equalization_msg_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	label_pos[do_equalization_msg_label] = widgetpos_temp

	#plot
	plot = BooleanVar()
	plot_checkbutton = Checkbutton(canvas2, text="Plot results", variable=plot, command=refresh2)
	ToolTip(plot_checkbutton, 'Create graphs in your recordings folder (will increase processing time)')
	_, pos2, imp_maxwidth, imp_maxheight = pack(plot_checkbutton, pos2, imp_maxwidth, imp_maxheight)

	show_adv = BooleanVar()
	_, pos2, imp_maxwidth, imp_maxheight = pack(Checkbutton(canvas2, text='Advanced options', variable=show_adv, command=refresh2), pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos = {} #save advanced options widgets' positions to show/hide

	#resample
	fs_check = BooleanVar()
	fs_checkbutton = Checkbutton(canvas2, text="Resample to (Hz)", variable=fs_check, command=refresh2)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(fs_checkbutton, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[fs_checkbutton] = widgetpos_temp
	sample_rates = [44100, 48000, 88200, 96000, 176400, 192000, 352000, 384000]
	fs = IntVar(value=48000)
	fs_optionmenu = OptionMenu(canvas2, fs, *sample_rates)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(fs_optionmenu, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[fs_optionmenu] = widgetpos_temp

	#target level
	target_level_label = Label(canvas2, text='Target level (dB)')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(target_level_label, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[target_level_label] = widgetpos_temp
	target_level = StringVar()
	target_level_entry = Entry(canvas2, textvariable=target_level, width=7, validate='key', vcmd=(root.register(validate_double), '%P'))
	ToolTip(target_level_label, 'Normalize the average output BRIR level to the given numeric value. This makes it possible to compare HRIRs with somewhat similar loudness levels. Typically the desired level is several dB negative such as -12.5')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(target_level_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[target_level_entry] = widgetpos_temp

	#bass boost
	bass_boost_gain_label = Label(canvas2, text='Bass boost (dB)')
	ToolTip(bass_boost_gain_label, 'Bass boost shelf')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_gain_label, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[bass_boost_gain_label] = widgetpos_temp
	bass_boost_gain = DoubleVar()
	bass_boost_gain_entry = Entry(canvas2, textvariable=bass_boost_gain, width=7, validate='key', vcmd=(root.register(validate_double), '%P'))
	ToolTip(bass_boost_gain_entry, 'Gain')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_gain_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[bass_boost_gain_entry] = widgetpos_temp

	bass_boost_fc_label = Label(canvas2, text='Fc')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_fc_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[bass_boost_fc_label] = widgetpos_temp
	bass_boost_fc = IntVar(value=105)
	bass_boost_fc_entry = Entry(canvas2, textvariable=bass_boost_fc, width=7, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_fc_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[bass_boost_fc_entry] = widgetpos_temp
	ToolTip(bass_boost_fc_entry, 'Center Freq')

	bass_boost_q_label = Label(canvas2, text='Q')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_q_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[bass_boost_q_label] = widgetpos_temp
	bass_boost_q = DoubleVar(value=0.76)
	bass_boost_q_entry = Entry(canvas2, textvariable=bass_boost_q, width=7, validate='key', vcmd=(root.register(validate_double), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(bass_boost_q_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[bass_boost_q_entry] = widgetpos_temp
	ToolTip(bass_boost_q_entry, 'Quality')

	#tilt
	tilt_label = Label(canvas2, text='Tilt (dB)')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(tilt_label, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[tilt_label] = widgetpos_temp
	tilt = DoubleVar()
	tilt_entry = Entry(canvas2, textvariable=tilt, width=7, validate='key', vcmd=(root.register(validate_double), '%P'))
	ToolTip(tilt_label, 'Target tilt in dB/octave. Positive value (upwards slope) will result in brighter frequency response and negative value (downwards slope) will result in darker frequency response.')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(tilt_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[tilt_entry] = widgetpos_temp

	#Channel Balance
	channel_balance_label = Label(canvas2, text='Channel Balance')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(channel_balance_label, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[channel_balance_label] = widgetpos_temp
	channel_balances = ['none', 'trend', 'mids', 'avg', 'min', 'left', 'right', 'number']
	channel_balance = StringVar(value=channel_balances[0])
	channel_balance.trace('w', lambda *args: refresh2())
	channel_balance_optionmenu = OptionMenu(canvas2, channel_balance, *channel_balances)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(channel_balance_optionmenu, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[channel_balance_optionmenu] = widgetpos_temp
	ToolTip(channel_balance_label, 'Channel balance correction by equalizing left and right ear results to the same level or frequency response. "trend" equalizes right side by the difference trend of right and left side. "left" equalizes right side to left side fr, "right" equalizes left side to right side fr, "avg" equalizes both to the average fr, "min" equalizes both to the minimum of left and right side frs. Number values will boost or attenuate right side relative to left side by the number of dBs. "mids" is the same as the numerical values but guesses the value automatically from mid frequency levels.')
	channel_balance_db = IntVar(value=0)
	channel_balance_db_entry = Entry(canvas2, textvariable=channel_balance_db, width=5, validate='key', vcmd=(root.register(validate_double), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(channel_balance_db_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[channel_balance_db_entry] = widgetpos_temp
	channel_balance_db_label = Label(canvas2, text='dB')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(channel_balance_db_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[channel_balance_db_label] = widgetpos_temp

	#decay
	decay_label = Label(canvas2, text='Decay (ms)')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_label, pos2, imp_maxwidth, imp_maxheight)
	adv_options_pos[decay_label] = widgetpos_temp
	decay = StringVar()
	decay_entry = Entry(canvas2, textvariable=decay, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	ToolTip(decay_label, 'Target decay time to reach -60 dB. When natural decay time is longer than the target decay time, a downward slope will be applied to decay tail. Decay cannot be increased with this. Can help reduce ringing in the room without having to do any physical room treatments.')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_entry, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_entry] = widgetpos_temp
	decay_per_channel = BooleanVar()
	decay_per_channel_checkbutton = Checkbutton(canvas2, text="per channel", variable=decay_per_channel, command=refresh2)
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_per_channel_checkbutton, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_per_channel_checkbutton] = widgetpos_temp

	decay_fl_label = Label(canvas2, text='FL')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fl_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fl_label] = widgetpos_temp
	decay_fl = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fl, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fl] = widgetpos_temp
	decay_fc_label = Label(canvas2, text='FC')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fc_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fc_label] = widgetpos_temp
	decay_fc = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fc, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fc] = widgetpos_temp
	decay_fr_label = Label(canvas2, text='FR')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fr_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fr_label] = widgetpos_temp
	decay_fr = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_fr, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_fr] = widgetpos_temp
	decay_sl_label = Label(canvas2, text='SL')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_sl_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_sl_label] = widgetpos_temp
	decay_sl = Entry(canvas2,  width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_sl, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_sl] = widgetpos_temp
	decay_sr_label = Label(canvas2, text='SR')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_sr_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_sr_label] = widgetpos_temp
	decay_sr = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_sr, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_sr] = widgetpos_temp
	decay_bl_label = Label(canvas2, text='BL')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_bl_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_bl_label] = widgetpos_temp
	decay_bl = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_bl, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_bl] = widgetpos_temp
	decay_br_label = Label(canvas2, text='BR')
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_br_label, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_br_label] = widgetpos_temp
	decay_br = Entry(canvas2, width=5, validate='key', vcmd=(root.register(validate_int), '%P'))
	widgetpos_temp, pos2, imp_maxwidth, imp_maxheight = pack(decay_br, pos2, imp_maxwidth, imp_maxheight, samerow=True)
	adv_options_pos[decay_br] = widgetpos_temp

	decay_labels = []
	decay_entries = []
	decay_labels.append(decay_fl_label)
	decay_labels.append(decay_fc_label)
	decay_labels.append(decay_fr_label)
	decay_labels.append(decay_sl_label)
	decay_labels.append(decay_sr_label)
	decay_labels.append(decay_bl_label)
	decay_labels.append(decay_br_label)
	decay_entries.append(decay_fl)
	decay_entries.append(decay_fc)
	decay_entries.append(decay_fr)
	decay_entries.append(decay_sl)
	decay_entries.append(decay_sr)
	decay_entries.append(decay_bl)
	decay_entries.append(decay_br)

	#impulcify button
	def impulcify():
		args = {'dir_path': dir_path.get(), 'test_signal': test_signal.get(), 'plot':plot.get(), 'do_room_correction': do_room_correction.get(), 'do_headphone_compensation':do_headphone_compensation.get(), 'do_equalization':do_equalization.get()}
		if do_room_correction.get():
			args['room_target'] = room_target.get() if room_target.get() else None
			args['room_mic_calibration'] = room_mic_calibration.get() if room_mic_calibration.get() else None
			args['specific_limit'] = specific_limit.get()
			args['generic_limit'] = generic_limit.get()
			args['fr_combination_method'] = fr_combination_method.get()
		if show_adv.get():
			args['fs'] = fs.get() if fs_check.get() else None
			args['target_level'] = float(target_level.get()) if target_level.get() else None
			args['channel_balance'] = channel_balance_db.get() if channel_balance.get() == 'number' else (channel_balance.get() if channel_balance.get() != 'none' else None)
			args['bass_boost_gain'] = bass_boost_gain.get()
			args['bass_boost_fc'] = bass_boost_fc.get()
			args['bass_boost_q'] = bass_boost_q.get()
			args['tilt'] = tilt.get()
			if decay_per_channel.get():
				args['decay'] = {decay_labels[i].cget('text') : float(decay_entries[i].get()) / 1000 for i in range(7) if decay_entries[i].get()}
			elif decay.get():
				args['decay'] = {decay_labels[i].cget('text') : float(decay.get()) / 1000 for i in range(7)}
		print(args) #debug args
		impulcifer.main(**args)
		showinfo('Done!', 'Generated files, check recordings folder.')
	_, pos2, imp_maxwidth, imp_maxheight = pack(Button(canvas2, text='GENERATE', command=impulcify), pos2, imp_maxwidth, imp_maxheight)

	canvas2.config(width=imp_maxwidth, height=imp_maxheight)
	canvas2.pack()
	window2.geometry(str(imp_maxwidth) + 'x' + str(imp_maxheight) + '+' + str(canvas1_final_width) + '+0')
	window2.resizable(False, False)
	refresh2(changedpath=True)
	root.mainloop()

if __name__ == "__main__":
	main_gui()
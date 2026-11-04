# Reference tables

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

Generated from the pinned source in `reference/quake/WinQuake` at commit `bf4ac42`. For what these features do, see [feature-catalog.md](feature-catalog.md).

The tables cover the code as written, including every platform backend. Items gated by the disabled `QUAKE2` and `IDGODS` flags are left out. See [baseline.md](baseline.md) for the flag settings.

## Console commands (148)

`+name` commands also have a matching `-name` release command.

**Host and game commands:** `begin`, `changelevel`, `color`, `connect`, `demos`, `fly`, `give`, `god`, `kick`, `kill`, `load`, `map`, `mcache`, `name`, `noclip`, `notarget`, `pause`, `ping`, `prespawn`, `quit`, `reconnect`, `restart`, `save`, `say`, `say_team`, `spawn`, `startdemos`, `status`, `stopdemo`, `tell`, `version`, `viewframe`, `viewmodel`, `viewnext`, `viewprev`

**Foundation:** `alias`, `cmd`, `echo`, `exec`, `flush`, `path`, `stuffcmds`, `wait`

**QuakeC VM:** `edict`, `edictcount`, `edicts`, `profile`

**Client:** `+attack`, `+back`, `+forward`, `+jump`, `+klook`, `+left`, `+lookdown`, `+lookup`, `+mlook`, `+movedown`, `+moveleft`, `+moveright`, `+moveup`, `+right`, `+speed`, `+strafe`, `+use`, `bf`, `centerview`, `disconnect`, `entities`, `impulse`, `playdemo`, `record`, `stop`, `timedemo`, `v_cshift`

**UI:** `+showscores`, `bind`, `clear`, `help`, `menu_keys`, `menu_load`, `menu_main`, `menu_multiplayer`, `menu_options`, `menu_quit`, `menu_save`, `menu_setup`, `menu_singleplayer`, `menu_video`, `messagemode`, `messagemode2`, `toggleconsole`, `togglemenu`, `unbind`, `unbindall`

**Sound:** `play`, `playvol`, `soundinfo`, `soundlist`, `stopsound`

**Network:** `ban`, `listen`, `maxplayers`, `net_stats`, `port`, `slist`, `test`, `test2`

**OpenGL renderer:** `envmap`, `gl_texturemode`, `pointfile`, `screenshot`, `sizedown`, `sizeup`, `timerefresh`

**Platform backends:** `force_centerview`, `joyadvancedupdate`, `vid_debug`, `vid_describecurrentmode`, `vid_describemode`, `vid_describemodes`, `vid_forcemode`, `vid_fullscreen`, `vid_minimize`, `vid_nummodes`, `vid_windowed`

**Legacy platforms:** `cd`, `gamma`, `sbinfo`, `toggle_auxlook`, `vid_testmode`

## Cvars (211)

**Saved** means the value is written to `config.cfg`. **Server** means changes are announced to connected players.

### Host and game commands

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `coop` | `0` |  |  | `host.c` |
| `deathmatch` | `0` |  |  | `host.c` |
| `developer` | `0` |  |  | `host.c` |
| `fraglimit` | `0` |  | yes | `host.c` |
| `host_framerate` | `0` |  |  | `host.c` |
| `host_speeds` | `0` |  |  | `host.c` |
| `noexit` | `0` |  | yes | `host.c` |
| `pausable` | `1` |  |  | `host.c` |
| `samelevel` | `0` |  |  | `host.c` |
| `serverprofile` | `0` |  |  | `host.c` |
| `skill` | `1` |  |  | `host.c` |
| `sys_ticrate` | `0.05` |  |  | `host.c` |
| `teamplay` | `0` |  | yes | `host.c` |
| `temp1` | `0` |  |  | `host.c` |
| `timelimit` | `0` |  | yes | `host.c` |

### Foundation

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `cmdline` | `0` |  | yes | `common.c` |
| `registered` | `0` |  |  | `common.c` |

### Server

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `edgefriction` | `2` |  |  | `sv_user.c` |
| `sv_accelerate` | `10` |  |  | `sv_user.c` |
| `sv_friction` | `4` |  | yes | `sv_phys.c` |
| `sv_gravity` | `800` |  | yes | `sv_phys.c` |
| `sv_idealpitchscale` | `0.8` |  |  | `sv_user.c` |
| `sv_maxspeed` | `320` |  | yes | `sv_user.c` |
| `sv_maxvelocity` | `2000` |  |  | `sv_phys.c` |
| `sv_nostep` | `0` |  |  | `sv_phys.c` |
| `sv_stopspeed` | `100` |  |  | `sv_phys.c` |

### QuakeC VM

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `gamecfg` | `0` |  |  | `pr_edict.c` |
| `nomonsters` | `0` |  |  | `pr_edict.c` |
| `saved1` | `0` | yes |  | `pr_edict.c` |
| `saved2` | `0` | yes |  | `pr_edict.c` |
| `saved3` | `0` | yes |  | `pr_edict.c` |
| `saved4` | `0` | yes |  | `pr_edict.c` |
| `savedgamecfg` | `0` | yes |  | `pr_edict.c` |
| `scratch1` | `0` |  |  | `pr_edict.c` |
| `scratch2` | `0` |  |  | `pr_edict.c` |
| `scratch3` | `0` |  |  | `pr_edict.c` |
| `scratch4` | `0` |  |  | `pr_edict.c` |
| `sv_aim` | `0.93` |  |  | `pr_cmds.c` |

### Client

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `_cl_color` | `0` | yes |  | `cl_main.c` |
| `_cl_name` | `player` | yes |  | `cl_main.c` |
| `chase_active` | `0` |  |  | `chase.c` |
| `chase_back` | `100` |  |  | `chase.c` |
| `chase_right` | `0` |  |  | `chase.c` |
| `chase_up` | `16` |  |  | `chase.c` |
| `cl_anglespeedkey` | `1.5` |  |  | `cl_input.c` |
| `cl_backspeed` | `200` | yes |  | `cl_input.c` |
| `cl_bob` | `0.02` |  |  | `view.c` |
| `cl_bobcycle` | `0.6` |  |  | `view.c` |
| `cl_bobup` | `0.5` |  |  | `view.c` |
| `cl_crossx` | `0` |  |  | `view.c` |
| `cl_crossy` | `0` |  |  | `view.c` |
| `cl_forwardspeed` | `200` | yes |  | `cl_input.c` |
| `cl_movespeedkey` | `2.0` |  |  | `cl_input.c` |
| `cl_nolerp` | `0` |  |  | `cl_main.c` |
| `cl_pitchspeed` | `150` |  |  | `cl_input.c` |
| `cl_rollangle` | `2.0` |  |  | `view.c` |
| `cl_rollspeed` | `200` |  |  | `view.c` |
| `cl_shownet` | `0` |  |  | `cl_main.c` |
| `cl_sidespeed` | `350` |  |  | `cl_input.c` |
| `cl_upspeed` | `200` |  |  | `cl_input.c` |
| `cl_yawspeed` | `140` |  |  | `cl_input.c` |
| `crosshair` | `0` | yes |  | `view.c` |
| `gamma` | `1` | yes |  | `view.c` |
| `gl_cshiftpercent` | `100` |  |  | `view.c` |
| `lcd_x` | `0` |  |  | `view.c` |
| `lcd_yaw` | `0` |  |  | `view.c` |
| `lookspring` | `0` | yes |  | `cl_main.c` |
| `lookstrafe` | `0` | yes |  | `cl_main.c` |
| `m_forward` | `1` | yes |  | `cl_main.c` |
| `m_pitch` | `0.022` | yes |  | `cl_main.c` |
| `m_side` | `0.8` | yes |  | `cl_main.c` |
| `m_yaw` | `0.022` | yes |  | `cl_main.c` |
| `scr_ofsx` | `0` |  |  | `view.c` |
| `scr_ofsy` | `0` |  |  | `view.c` |
| `scr_ofsz` | `0` |  |  | `view.c` |
| `sensitivity` | `3` | yes |  | `cl_main.c` |
| `v_centermove` | `0.15` |  |  | `view.c` |
| `v_centerspeed` | `500` |  |  | `view.c` |
| `v_idlescale` | `0` |  |  | `view.c` |
| `v_ipitch_cycle` | `1` |  |  | `view.c` |
| `v_ipitch_level` | `0.3` |  |  | `view.c` |
| `v_iroll_cycle` | `0.5` |  |  | `view.c` |
| `v_iroll_level` | `0.1` |  |  | `view.c` |
| `v_iyaw_cycle` | `2` |  |  | `view.c` |
| `v_iyaw_level` | `0.3` |  |  | `view.c` |
| `v_kickpitch` | `0.6` |  |  | `view.c` |
| `v_kickroll` | `0.6` |  |  | `view.c` |
| `v_kicktime` | `0.5` |  |  | `view.c` |

### UI

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `con_notifytime` | `3` |  |  | `console.c` |

### Sound

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `_snd_mixahead` | `0.1` | yes |  | `snd_dma.c` |
| `ambient_fade` | `100` |  |  | `snd_dma.c` |
| `ambient_level` | `0.3` |  |  | `snd_dma.c` |
| `bgmbuffer` | `4096` |  |  | `snd_dma.c` |
| `bgmvolume` | `1` | yes |  | `snd_dma.c` |
| `loadas8bit` | `0` |  |  | `snd_dma.c` |
| `nosound` | `0` |  |  | `snd_dma.c` |
| `precache` | `1` |  |  | `snd_dma.c` |
| `snd_noextraupdate` | `0` |  |  | `snd_dma.c` |
| `snd_show` | `0` |  |  | `snd_dma.c` |
| `volume` | `0.7` | yes |  | `snd_dma.c` |

### Network

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `_config_com_baud` | `57600` | yes |  | `net_main.c` |
| `_config_com_irq` | `4` | yes |  | `net_main.c` |
| `_config_com_modem` | `1` | yes |  | `net_main.c` |
| `_config_com_port` | `0x3f8` | yes |  | `net_main.c` |
| `_config_modem_clear` | `ATZ` | yes |  | `net_main.c` |
| `_config_modem_dialtype` | `T` | yes |  | `net_main.c` |
| `_config_modem_hangup` | `AT H` | yes |  | `net_main.c` |
| `_config_modem_init` | `(empty)` | yes |  | `net_main.c` |
| `hostname` | `UNNAMED` |  |  | `net_main.c` |
| `net_messagetimeout` | `300` |  |  | `net_main.c` |

### Software renderer

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `d_mipcap` | `0` |  |  | `d_init.c` |
| `d_mipscale` | `1` |  |  | `d_init.c` |
| `d_subdiv16` | `1` |  |  | `d_init.c` |
| `r_aliastransadj` | `100` |  |  | `r_main.c` |
| `r_aliastransbase` | `200` |  |  | `r_main.c` |
| `r_ambient` | `0` |  |  | `r_main.c` |
| `r_clearcolor` | `2` |  |  | `r_main.c` |
| `r_drawflat` | `0` |  |  | `r_main.c` |
| `r_draworder` | `0` |  |  | `r_main.c` |
| `r_dspeeds` | `0` |  |  | `r_main.c` |
| `r_graphheight` | `10` |  |  | `r_main.c` |
| `r_maxedges` | `0` |  |  | `r_main.c` |
| `r_maxsurfs` | `0` |  |  | `r_main.c` |
| `r_numedges` | `0` |  |  | `r_main.c` |
| `r_numsurfs` | `0` |  |  | `r_main.c` |
| `r_polymodelstats` | `0` |  |  | `r_main.c` |
| `r_reportedgeout` | `0` |  |  | `r_main.c` |
| `r_reportsurfout` | `0` |  |  | `r_main.c` |
| `r_timegraph` | `0` |  |  | `r_main.c` |
| `r_waterwarp` | `1` |  |  | `r_main.c` |

### OpenGL renderer

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `fov` | `90` |  |  | `gl_screen.c` |
| `gl_affinemodels` | `0` |  |  | `gl_rmain.c` |
| `gl_clear` | `0` |  |  | `gl_rmain.c` |
| `gl_cull` | `1` |  |  | `gl_rmain.c` |
| `gl_doubleeys` | `1` |  |  | `gl_rmain.c` |
| `gl_finish` | `0` |  |  | `gl_rmain.c` |
| `gl_flashblend` | `1` |  |  | `gl_rmain.c` |
| `gl_keeptjunctions` | `0` |  |  | `gl_rmain.c` |
| `gl_max_size` | `1024` |  |  | `gl_draw.c` |
| `gl_nobind` | `0` |  |  | `gl_draw.c` |
| `gl_nocolors` | `0` |  |  | `gl_rmain.c` |
| `gl_picmip` | `0` |  |  | `gl_draw.c` |
| `gl_playermip` | `0` |  |  | `gl_rmain.c` |
| `gl_polyblend` | `1` |  |  | `gl_rmain.c` |
| `gl_reporttjunctions` | `0` |  |  | `gl_rmain.c` |
| `gl_smoothmodels` | `1` |  |  | `gl_rmain.c` |
| `gl_subdivide_size` | `128` | yes |  | `gl_model.c` |
| `gl_texsort` | `1` |  |  | `gl_rmain.c` |
| `gl_triplebuffer` | `1` | yes |  | `gl_screen.c` |
| `r_drawentities` | `1` |  |  | `gl_rmain.c` |
| `r_drawviewmodel` | `1` |  |  | `gl_rmain.c` |
| `r_dynamic` | `1` |  |  | `gl_rmain.c` |
| `r_fullbright` | `0` |  |  | `gl_rmain.c` |
| `r_lightmap` | `0` |  |  | `gl_rmain.c` |
| `r_mirroralpha` | `1` |  |  | `gl_rmain.c` |
| `r_norefresh` | `0` |  |  | `gl_rmain.c` |
| `r_novis` | `0` |  |  | `gl_rmain.c` |
| `r_shadows` | `0` |  |  | `gl_rmain.c` |
| `r_speeds` | `0` |  |  | `gl_rmain.c` |
| `r_wateralpha` | `1` |  |  | `gl_rmain.c` |
| `scr_centertime` | `2` |  |  | `gl_screen.c` |
| `scr_conspeed` | `300` |  |  | `gl_screen.c` |
| `scr_printspeed` | `8` |  |  | `gl_screen.c` |
| `showpause` | `1` |  |  | `gl_screen.c` |
| `showram` | `1` |  |  | `gl_screen.c` |
| `showturtle` | `0` |  |  | `gl_screen.c` |
| `viewsize` | `100` | yes |  | `gl_screen.c` |

### Platform backends

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `_vid_default_mode` | `0` | yes |  | `gl_vidnt.c` |
| `_vid_default_mode_win` | `3 or 1` | yes |  | `gl_vidnt.c` |
| `_vid_wait_override` | `0` | yes |  | `gl_vidnt.c` |
| `_windowed_mouse` | `1 or 0` | yes |  | `gl_vidnt.c` |
| `gl_ztrick` | `1` |  |  | `gl_vidlinux.c` |
| `in_dgamouse` | `1` |  |  | `gl_vidlinuxglx.c` |
| `in_mouse` | `1` |  |  | `gl_vidlinuxglx.c` |
| `joyadvanced` | `0` |  |  | `in_win.c` |
| `joyadvaxisr` | `0` |  |  | `in_win.c` |
| `joyadvaxisu` | `0` |  |  | `in_win.c` |
| `joyadvaxisv` | `0` |  |  | `in_win.c` |
| `joyadvaxisx` | `0` |  |  | `in_win.c` |
| `joyadvaxisy` | `0` |  |  | `in_win.c` |
| `joyadvaxisz` | `0` |  |  | `in_win.c` |
| `joyforwardsensitivity` | `-1.0` |  |  | `in_win.c` |
| `joyforwardthreshold` | `0.15` |  |  | `in_win.c` |
| `joyname` | `joystick` |  |  | `in_win.c` |
| `joypitchsensitivity` | `1.0` |  |  | `in_win.c` |
| `joypitchthreshold` | `0.15` |  |  | `in_win.c` |
| `joysidesensitivity` | `-1.0` |  |  | `in_win.c` |
| `joysidethreshold` | `0.15` |  |  | `in_win.c` |
| `joywwhack1` | `0.0` |  |  | `in_win.c` |
| `joywwhack2` | `0.0` |  |  | `in_win.c` |
| `joyyawsensitivity` | `-1.0` |  |  | `in_win.c` |
| `joyyawthreshold` | `0.15` |  |  | `in_win.c` |
| `m_filter` | `1 or 0` |  |  | `gl_vidlinux.c` |
| `sys_linerefresh` | `0` |  |  | `sys_linux.c` |
| `vid_config_x` | `800` | yes |  | `gl_vidnt.c` |
| `vid_config_y` | `600` | yes |  | `gl_vidnt.c` |
| `vid_mode` | `5 or 0` |  |  | `gl_vidlinux.c` |
| `vid_nopageflip` | `0` | yes |  | `gl_vidnt.c` |
| `vid_redrawfull` | `0` |  |  | `gl_vidlinux.c` |
| `vid_stretch_by_2` | `1` | yes |  | `gl_vidnt.c` |
| `vid_wait` | `0` |  |  | `gl_vidnt.c` |
| `vid_waitforrefresh` | `0` | yes |  | `gl_vidlinux.c` |

### Legacy platforms

| Cvar | Default | Saved | Server | File |
|---|---|---|---|---|
| `auxlook` | `1` | yes |  | `in_dos.c` |
| `block_switch` | `0` | yes |  | `vid_dos.c` |
| `joybuttons` | `4` | yes |  | `in_dos.c` |
| `joystick` | `1 or 0` |  |  | `in_dos.c` |
| `pixel_multiply` | `2` | yes |  | `vid_sunxil.c` |
| `vid_fullscreen_mode` | `3` | yes |  | `vid_dos.c` |
| `vid_window_x` | `0` | yes |  | `vid_dos.c` |
| `vid_window_y` | `0` | yes |  | `vid_dos.c` |
| `vid_windowed_mode` | `0` | yes |  | `vid_dos.c` |

## Command-line switches (104)

This counts only `COM_CheckParm` calls. `-safe` is handled separately in `common.c` and expands to a fixed list of switches.

**Host and game commands:** `-dedicated`, `-listen`, `-minmemory`, `-playback`, `-record`

**Foundation:** `-basedir`, `-cachedir`, `-game`, `-hipnotic`, `-path`, `-proghack`, `-rogue`, `-zone`

**UI:** `-condebug`

**Sound:** `-nosound`, `-simsound`

**Network:** `-ip`, `-ipxport`, `-nolan`, `-port`, `-udpport`

**Software renderer:** `-particles`, `-surfcachesize`

**OpenGL renderer:** `-lm_1`, `-lm_2`, `-lm_4`, `-lm_a`, `-lm_i`

**Platform backends:** `-HCHILD`, `-HFILE`, `-HPARENT`, `-allow360`, `-bpp`, `-cddev`, `-conheight`, `-conwidth`, `-current`, `-d`, `-dibonly`, `-dinput`, `-force`, `-fullsbar`, `-gamma`, `-gl11`, `-h`, `-height`, `-mdev`, `-mode`, `-mrate`, `-no8bit`, `-noadjustaspect`, `-noautostretch`, `-nodd`, `-noddraw`, `-nodirectdraw`, `-noforcemaccel`, `-noforcemparms`, `-noforcemspd`, `-noforcevga`, `-nofulldib`, `-nokbd`, `-nomtex`, `-nostdout`, `-notriplebuf`, `-novbeaf`, `-novesa`, `-nowd`, `-nowindirect`, `-primarysound`, `-resetwinpos`, `-sndbits`, `-sndmono`, `-sndspeed`, `-sndstereo`, `-snoforceformat`, `-startwindowed`, `-w`, `-wavonly`, `-width`, `-window`

**Legacy platforms:** `-cdmediacheck`, `-control`, `-count_frames`, `-dsp`, `-heapsize`, `-mem`, `-mpath`, `-mt`, `-no_mt`, `-nocdaudio`, `-noipx`, `-nojoy`, `-nomouse`, `-noserial`, `-noudp`, `-sspeed`, `-starttime`, `-stdvid`, `-verbose`, `-visualid`, `-winlock`, `-winlockunlock`, `-winmem`, `-winsize`

## QuakeC builtins (79 slots, 65 live)

Slot numbers are the ABI. Names and signatures come from the GPL v1.01 `defs.qc` in id's Quake-Tools. Unlisted slots are `PF_Fixme` placeholders that raise a runtime error.

| Slot | QuakeC name | Signature | Engine function |
|---:|---|---|---|
| 1 | `makevectors` | `void(vector ang)` | `PF_makevectors` |
| 2 | `setorigin` | `void(entity e, vector o)` | `PF_setorigin` |
| 3 | `setmodel` | `void(entity e, string m)` | `PF_setmodel` |
| 4 | `setsize` | `void(entity e, vector min, vector max)` | `PF_setsize` |
| 6 | `break` | `void()` | `PF_break` |
| 7 | `random` | `float()` | `PF_random` |
| 8 | `sound` | `void(entity e, float chan, string samp, float vol, float atten)` | `PF_sound` |
| 9 | `normalize` | `vector(vector v)` | `PF_normalize` |
| 10 | `error` | `void(string e)` | `PF_error` |
| 11 | `objerror` | `void(string e)` | `PF_objerror` |
| 12 | `vlen` | `float(vector v)` | `PF_vlen` |
| 13 | `vectoyaw` | `float(vector v)` | `PF_vectoyaw` |
| 14 | `spawn` | `entity()` | `PF_Spawn` |
| 15 | `remove` | `void(entity e)` | `PF_Remove` |
| 16 | `traceline` | `void(vector v1, vector v2, float nomonsters, entity forent)` | `PF_traceline` |
| 17 | `checkclient` | `entity()` | `PF_checkclient` |
| 18 | `find` | `entity(entity start, .string fld, string match)` | `PF_Find` |
| 19 | `precache_sound` | `string(string s)` | `PF_precache_sound` |
| 20 | `precache_model` | `string(string s)` | `PF_precache_model` |
| 21 | `stuffcmd` | `void(entity client, string s)` | `PF_stuffcmd` |
| 22 | `findradius` | `entity(vector org, float rad)` | `PF_findradius` |
| 23 | `bprint` | `void(string s)` | `PF_bprint` |
| 24 | `sprint` | `void(entity client, string s)` | `PF_sprint` |
| 25 | `dprint` | `void(string s)` | `PF_dprint` |
| 26 | `ftos` | `string(float f)` | `PF_ftos` |
| 27 | `vtos` | `string(vector v)` | `PF_vtos` |
| 28 | `coredump` | `void()` | `PF_coredump` |
| 29 | `traceon` | `void()` | `PF_traceon` |
| 30 | `traceoff` | `void()` | `PF_traceoff` |
| 31 | `eprint` | `void(entity e)` | `PF_eprint` |
| 32 | `walkmove` | `float(float yaw, float dist)` | `PF_walkmove` |
| 34 | `droptofloor` | `float(float yaw, float dist)` | `PF_droptofloor` |
| 35 | `lightstyle` | `void(float style, string value)` | `PF_lightstyle` |
| 36 | `rint` | `float(float v)` | `PF_rint` |
| 37 | `floor` | `float(float v)` | `PF_floor` |
| 38 | `ceil` | `float(float v)` | `PF_ceil` |
| 40 | `checkbottom` | `float(entity e)` | `PF_checkbottom` |
| 41 | `pointcontents` | `float(vector v)` | `PF_pointcontents` |
| 43 | `fabs` | `float(float f)` | `PF_fabs` |
| 44 | `aim` | `vector(entity e, float speed)` | `PF_aim` |
| 45 | `cvar` | `float(string s)` | `PF_cvar` |
| 46 | `localcmd` | `void(string s)` | `PF_localcmd` |
| 47 | `nextent` | `entity(entity e)` | `PF_nextent` |
| 48 | `particle` | `void(vector o, vector d, float color, float count)` | `PF_particle` |
| 49 | `ChangeYaw` | `void()` | `PF_changeyaw` |
| 51 | `vectoangles` | `vector(vector v)` | `PF_vectoangles` |
| 52 | `WriteByte` | `void(float to, float f)` | `PF_WriteByte` |
| 53 | `WriteChar` | `void(float to, float f)` | `PF_WriteChar` |
| 54 | `WriteShort` | `void(float to, float f)` | `PF_WriteShort` |
| 55 | `WriteLong` | `void(float to, float f)` | `PF_WriteLong` |
| 56 | `WriteCoord` | `void(float to, float f)` | `PF_WriteCoord` |
| 57 | `WriteAngle` | `void(float to, float f)` | `PF_WriteAngle` |
| 58 | `WriteString` | `void(float to, string s)` | `PF_WriteString` |
| 59 | `WriteEntity` | `void(float to, entity s)` | `PF_WriteEntity` |
| 67 | `movetogoal` | `void(float step)` | `SV_MoveToGoal` |
| 68 | `precache_file` | `string(string s)` | `PF_precache_file` |
| 69 | `makestatic` | `void(entity e)` | `PF_makestatic` |
| 70 | `changelevel` | `void(string s)` | `PF_changelevel` |
| 72 | `cvar_set` | `void(string var, string val)` | `PF_cvar_set` |
| 73 | `centerprint` | `void(entity client, string s)` | `PF_centerprint` |
| 74 | `ambientsound` | `void(vector pos, string samp, float vol, float atten)` | `PF_ambientsound` |
| 75 | `precache_model2` | `string(string s)` | `PF_precache_model` |
| 76 | `precache_sound2` | `string(string s)` | `PF_precache_sound` |
| 77 | `precache_file2` | `string(string s)` | `PF_precache_file` |
| 78 | `setspawnparms` | `void(entity e)` | `PF_setspawnparms` |

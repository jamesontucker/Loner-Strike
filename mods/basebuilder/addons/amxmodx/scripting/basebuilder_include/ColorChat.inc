// Minimal ColorChat compatibility shim (uses AMXX 1.9+/1.10 client_print_color)
#if defined _colorchat_included
	#endinput
#endif
#define _colorchat_included

enum Color
{
	NORMAL = 1,
	GREEN,
	TEAM_COLOR,
	GREY,
	RED,
	BLUE
}

stock ColorChat(id, Color:type, const msg[], any:...)
{
	new szMessage[191];
	switch(type)
	{
		case NORMAL: szMessage[0] = 0x01;
		case GREEN: szMessage[0] = 0x04;
		default: szMessage[0] = 0x03;
	}
	vformat(szMessage[1], charsmax(szMessage) - 1, msg, 4);
	new sender = print_team_default;
	switch(type)
	{
		case GREY: sender = print_team_grey;
		case RED: sender = print_team_red;
		case BLUE: sender = print_team_blue;
	}
	client_print_color(id, sender, "%s", szMessage);
}

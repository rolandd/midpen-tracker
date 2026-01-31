// FOUC prevention: hide body if user is logged in, before any content paints
if (document.cookie.includes('midpen_logged_in=1')) {
	document.documentElement.classList.add('auth-pending');
}

// FOUC prevention: hide body if user is logged in, before any content paints
console.log('[FOUC] Checking cookies:', document.cookie);
if (document.cookie.indexOf('midpen_logged_in') !== -1) {
	console.log('[FOUC] Hiding body');
	document.documentElement.classList.add('auth-pending');
} else {
	console.log('[FOUC] Not logged in');
}

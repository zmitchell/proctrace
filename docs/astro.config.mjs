import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	integrations: [
		starlight({
			title: 'proctrace',
			social: {
				github: 'https://github.com/zmitchell/proctrace',
			},
			sidebar: [
				{
					label: 'Guides',
					items: [
						'guides/getting-started',
						'guides/raw-recordings',
						'guides/known-issues'
					]
				},
				{
					label: 'Reference',
					autogenerate: { directory: 'reference' },
				},
			],
		}),
	],
});

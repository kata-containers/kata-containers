# Blog Post Contributor Guide

This section describes the guidelines for contributing new blog posts to the
Kata Containers website.

## Share your stories on the Kata Containers website

Are you experimenting with Kata Containers or have it deployed in production and
would like to share your story as a case study? Do you have a use case that
Kata Containers can make more secure, but the world doesn't know it yet? Do you
have features in the runtime that you like and would like to highlight? Do you
have a Kata Containers demo that you would like to draw attention to?

Share your Kata Containers story on the [Kata Containers blog](https://www.katacontainers.io/blog/)!
You are only a few steps away...

### Kata Containers website source

Like the rest of the Kata Containers artifacts, the project’s website code and
content are stored in a [GitHub repository](https://github.com/kata-containers/www.katacontainers.io).

The blog posts are written using markdown language that is mainly plain text
with a few easy formatting conventions to create lists, add images or code blocks,
or format the text.

You can find many [cheat sheets](https://www.markdownguide.org/cheat-sheet/)
floating on the web to get in terms of the basic syntax. You can also check the
[source files of the already existing blog posts](https://github.com/kata-containers/www.katacontainers.io/tree/main/src/pages/blog),
where you will find examples of all the basic items that you will need for your
new entry.

### Create a new blog post

When you create a new blog post, you need to create a new file in the
[`src/pages/blog/` folder](https://github.com/kata-containers/www.katacontainers.io/tree/main/src/pages/blog)
with a `.md` extension.

The markdown file has a few formatting conventions in its header to capture the
title, author, publishing date and category of your blog post.

The header looks like the following:

```
  ---
  templateKey: blog-post
  title: The Title of Your Amazing Blog Post
  author: Your Name
  date: 2021-01-28T16:23:52.741Z
  category:
    - value: category-6-wjkXzEM2
      label: Features & Updates
  ---
```

The categories give the possibility to filter on the web page and see only the
blog posts that fall under one of the options. You can choose from the
following options:

* News & Announcements
* Features & Updates

The `Annual Report` category is reserved for the Kata Containers chapter in the
Open Infrastructure Annual report that we are also re-posting on the Kata
Containers website.

Once you filled out the above fields in the header and got your one-liner all
set, you can go ahead and type up the contents of your blog post using the
conventional markdown formatting.

If you have an image file to add, you need to place the file in the
`static/img` folder.

You can then insert the image into your blog post by using the following line:

```
  ![alt text](/img/the-file-name-of-your-image.jpg)
```

Once you are done with formatting your blog post and happy with the content, you
need to upload it to GitHub and create a pull request. You can do that by using
git commands on your laptop or you can also use the GitHub web interface to add
files to the repository and create a pull request when you are ready.

If you have an idea for a blog post and would like to get feedback from the
community about it or have any questions about the process, please reach out
on one of the community's [communication channels](https://katacontainers.io/community/).
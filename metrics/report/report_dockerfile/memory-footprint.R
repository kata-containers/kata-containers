#!/usr/bin/env Rscript
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Analyse the runtime component memory footprint data.

library(ggplot2)					# ability to plot nicely.
							# So we can plot multiple graphs
library(gridExtra)					# together.
suppressMessages(suppressWarnings(library(ggpubr)))	# for ggtexttable.
suppressMessages(library(jsonlite))			# to load the data.

testnames=c(
	"memory-footprint",
	"memory-footprint-ksm"
)

resultsfilesshort=c(
	"noKSM",
	"KSM"
)

data=c()
rstats=c()
rstats_names=c()

# For each set of results
for (currentdir in resultdirs) {
	count=1
	dirstats=c()
	# For the two different types of memory footprint measures
	for (testname in testnames) {
		# R seems not to like double path slashes '//' ?
		fname=paste(inputdir, currentdir, testname, '.json', sep="")
		if ( !file.exists(fname)) {
			warning(paste("Skipping non-existent file: ", fname))
			next
		}

		# Derive the name from the test result dirname
		datasetname=basename(currentdir)
		datasetvariant=resultsfilesshort[count]

		# Import the data
		fdata=fromJSON(fname)
		fdata=fdata[[testname]]
		# Copy the average result into a shorter, more accesible name
		fdata$Result=fdata$Results$average$Result
		fdata$variant=rep(datasetvariant, length(fdata$Result) )
		fdata$Runtime=rep(datasetname, length(fdata$Result) )
		fdata$Count=seq_len(length(fdata$Result))

		# Calculate some stats
		fdata.mean = mean(fdata$Result)
		fdata.min = min(fdata$Result)
		fdata.max = max(fdata$Result)
		fdata.sd = sd(fdata$Result)
		fdata.cov = (fdata.sd / fdata.mean) * 100

		# Store away the bits we need
		data=rbind(data, data.frame(
			Result=fdata$Result,
			Count=fdata$Count,
			Runtime=fdata$Runtime,
			variant=fdata$variant ) )

		# Store away some stats for the text table
		dirstats[count]=round(fdata.mean, digits=2)

		count = count + 1
	}
	rstats=rbind(rstats, dirstats)
	rstats_names=rbind(rstats_names, datasetname)
}

rstats=cbind(rstats_names, rstats)
unts=rep("Kb", length(resultdirs))

# If we have only 2 sets of results, then we can do some more
# stats math for the text table
if (length(resultdirs) == 2) {
	# This is a touch hard wired - but we *know* we only have two
	# datasets...
	diff=c("diff")
	difference = (as.double(rstats[2,2]) - as.double(rstats[1,2]))
	val = 100 * (difference/as.double(rstats[1,2]))
	diff[2] = round(val, digits=2)
	difference = (as.double(rstats[2,3]) - as.double(rstats[1,3]))
	val = 100 * (difference/as.double(rstats[1,3]))
	diff[3] = round(val, digits=2)
	rstats=rbind(rstats, diff)

	unts[3]="%"
}

rstats=cbind(rstats, unts)

# Set up the text table headers
colnames(rstats)=c("Results", resultsfilesshort, "Units")

# Build us a text table of numerical results
stats_plot = suppressWarnings(ggtexttable(data.frame(rstats),
	theme=ttheme(base_size=10),
	rows=NULL
	))

# plot how samples varioed over  'time'
point_plot <- ggplot() +
	geom_point( data=data, aes(Runtime, Result, color=variant), position=position_dodge(0.1)) +
	xlab("Dataset") +
	ylab("Size (Kb)") +
	ggtitle("Average PSS footprint", subtitle="per container") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

master_plot = grid.arrange(
	point_plot,
	stats_plot,
	nrow=1,
	ncol=2 )


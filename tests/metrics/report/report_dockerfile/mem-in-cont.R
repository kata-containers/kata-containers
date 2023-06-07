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
	"memory-footprint-inside-container"
)

data=c()
rstats=c()
rstats_rows=c()
rstats_cols=c()

# For each set of results
for (currentdir in resultdirs) {
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

		# Import the data
		fdata=fromJSON(fname)
		fdata=fdata[[testname]]
		# Copy the average result into a shorter, more accesible name
		fdata$requested=fdata$Results$memrequest$Result
		fdata$total=fdata$Results$memtotal$Result
		fdata$free=fdata$Results$memfree$Result
		fdata$avail=fdata$Results$memavailable$Result

		# And lets work out what % we have 'lost' between the amount requested
		# and the total the container actually sees.
		fdata$lost=fdata$requested - fdata$total
		fdata$pctotal= 100 * (fdata$lost/ fdata$requested)

		fdata$Runtime=rep(datasetname, length(fdata$Result) )

		# Store away the bits we need
		data=rbind(data, data.frame(
			Result=fdata$requested,
			Type="requested",
			Runtime=fdata$Runtime ))
			
		data=rbind(data, data.frame(
			Result=fdata$total,
			Type="total",
			Runtime=fdata$Runtime ))
			
		data=rbind(data, data.frame(
			Result=fdata$free,
			Type="free",
			Runtime=fdata$Runtime ))

		data=rbind(data, data.frame(
			Result=fdata$avail,
			Type="avail",
			Runtime=fdata$Runtime ))

		data=rbind(data, data.frame(
			Result=fdata$lost,
			Type="lost",
			Runtime=fdata$Runtime ))

		data=rbind(data, data.frame(
			Result=fdata$pctotal,
			Type="% consumed",
			Runtime=fdata$Runtime ))

		# Store away some stats for the text table
		dirstats=rbind(dirstats, round(fdata$requested, digits=2) )
		dirstats=rbind(dirstats, round(fdata$total, digits=2) )
		dirstats=rbind(dirstats, round(fdata$free, digits=2) )
		dirstats=rbind(dirstats, round(fdata$avail, digits=2) )
		dirstats=rbind(dirstats, round(fdata$lost, digits=2) )
		dirstats=rbind(dirstats, round(fdata$pctotal, digits=2) )
	}
	rstats=cbind(rstats, dirstats)
	rstats_cols=append(rstats_cols, datasetname)
}

rstats_rows=c("Requested", "Total", "Free", "Avail", "Consumed", "% Consumed")

unts=c("Kb", "Kb", "Kb", "Kb", "Kb", "%")
rstats=cbind(rstats, unts)
rstats_cols=append(rstats_cols, "Units")

# If we have only 2 sets of results, then we can do some more
# stats math for the text table
if (length(resultdirs) == 2) {
	# This is a touch hard wired - but we *know* we only have two
	# datasets...
	diff=c()
	# Just the first three entries - meaningless for the pctotal entry
	for (n in 1:5) {
		difference = (as.double(rstats[n,2]) - as.double(rstats[n,1]))
		val = 100 * (difference/as.double(rstats[n,1]))
		diff=rbind(diff, round(val, digits=2))
	}

	# Add a blank entry for the other entries
	diff=rbind(diff, "")
	rstats=cbind(rstats, diff)
	rstats_cols=append(rstats_cols, "Diff %")
}

# Build us a text table of numerical results
stats_plot = suppressWarnings(ggtexttable(data.frame(rstats),
	theme=ttheme(base_size=10),
	rows=rstats_rows, cols=rstats_cols
	))

bardata <- subset(data, Type %in% c("requested", "total", "free", "avail"))
# plot how samples varioed over  'time'
barplot <- ggplot() +
	geom_bar(data=bardata, aes(Type, Result, fill=Runtime), stat="identity", position="dodge") +
	xlab("Measure") +
	ylab("Size (Kb)") +
	ggtitle("In-container memory statistics") +
	ylim(0, NA) +
	theme(axis.text.x=element_text(angle=90))

master_plot = grid.arrange(
	barplot,
	stats_plot,
	nrow=2,
	ncol=1 )


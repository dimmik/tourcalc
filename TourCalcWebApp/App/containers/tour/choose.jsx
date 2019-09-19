import React from 'react';
import FetchHelper from './helpers.jsx';

import Radio from '@material-ui/core/Radio';
import RadioGroup from '@material-ui/core/RadioGroup';
import FormHelperText from '@material-ui/core/FormHelperText';
import FormControlLabel from '@material-ui/core/FormControlLabel';
import FormControl from '@material-ui/core/FormControl';
import FormLabel from '@material-ui/core/FormLabel';


export default class TourChoose extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            error: null,
            isLoaded: false,
            tours: []
        };
    }

    componentDidMount() {
        FetchHelper.fetchTours(this)
    }
    
    render() {
        const { error, isLoaded, tours } = this.state;
        if (error) {
            return <div>Error: {error.message}</div>;
        } else if (!isLoaded) {
            return <div>Loading...</div>;
        } else {
            // if not chosen - make first chosen
            if (this.props.chosenTour == null) {
                if (tours.length > 0) {
                    this.props.chooseTourAction(tours[0])
                }
            }
            return (
                <FormControl>
                    <RadioGroup aria-label="tours" name="tours" value={this.props.chosenTour.id} onChange={(e, v) => this.props.chooseTourAction(tours.find((t) => t.id == v)) }>
                        {
                            tours.map(tour => (
                                <FormControlLabel value={tour.id} control={<Radio />} label={tour.name} key={tour.id}/>
                        ))
                        }   
                    </RadioGroup>
                </FormControl>
                )
            
        }
    }
};